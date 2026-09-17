//! Kiro passive usage collector plus read-only subscription quota.
//!
//! Token usage is read from local Kiro session files. Subscription quota is
//! requested through the user's already-installed `kiro-cli` `/usage` command.
//! Newer Kiro CLI releases require a pseudo-terminal for `/usage`; on Windows
//! lnwdeck refuses to invoke those builds through a plain pipe because Kiro
//! would otherwise interpret `/usage` as a model prompt and spend credits.

use chrono::{DateTime, Datelike, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, UsageBatch,
    DEFAULT_FRESHNESS,
};
use lnwdeck_provider_runtime::command::{resolve_binary, run_command, CommandOutput};
use lnwdeck_provider_runtime::token_scan::{
    scan_directories, usage_events, ScanBounds, ScanReport,
};
use lnwdeck_provider_runtime::{
    AdapterDescriptor, AdapterHealth, AdapterHealthStatus, AuthKind, ChannelSupport,
    DetectionResult, Permission, ProviderAdapter, SourceKind,
};
use std::path::PathBuf;
use std::time::Duration;

const PROVIDER_ID: &str = "kiro_ai";
const ADAPTER_VERSION: &str = "0.3.0";
const DATA_SOURCE: &str = "local_jsonl";
const CLI_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CliVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

fn parse_cli_version(text: &str) -> Option<CliVersion> {
    for token in text.split_whitespace() {
        let clean = token.trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
        let mut parts = clean.split('.');
        let (Some(major), Some(minor), Some(patch_text)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (Ok(major), Ok(minor)) = (major.parse(), minor.parse()) else {
            continue;
        };
        let patch_digits = patch_text
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        let Ok(patch) = patch_digits.parse() else {
            continue;
        };
        return Some(CliVersion {
            major,
            minor,
            patch,
        });
    }
    None
}

fn requires_pty(version: CliVersion) -> bool {
    version.major > 2 || (version.major == 2 && version.minor >= 13)
}

fn strip_ansi(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == 0x1b {
            index += 1;
            if index < bytes.len() && bytes[index] == b'[' {
                index += 1;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if byte.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
            if index < bytes.len() && bytes[index] == b']' {
                index += 1;
                while index < bytes.len() && bytes[index] != 0x07 {
                    index += 1;
                }
                index = (index + 1).min(bytes.len());
                continue;
            }
            continue;
        }
        let ch = text[index..].chars().next().unwrap_or_default();
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn percent_before_marker(text: &str, marker: char) -> Option<f64> {
    let marker_index = text.find(marker)?;
    let prefix = &text[..marker_index];
    let digits = prefix
        .chars()
        .rev()
        .skip_while(|ch| ch.is_whitespace())
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    digits.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn parse_covered(text: &str) -> Option<(f64, f64)> {
    let covered = text.find(" covered")?;
    let open = text[..covered].rfind('(')?;
    let body = &text[open + 1..covered];
    let (used, total) = body.split_once(" of ")?;
    Some((used.trim().parse().ok()?, total.trim().parse().ok()?))
}

fn parse_reset(text: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let lower = text.to_ascii_lowercase();
    let start = lower.find("resets on ")? + "resets on ".len();
    let raw = text[start..]
        .split_whitespace()
        .next()?
        .trim_matches(|ch: char| ch == ',' || ch == '.');
    if raw.len() == 10 && raw.as_bytes().get(4) == Some(&b'-') {
        return DateTime::parse_from_rfc3339(&format!("{raw}T00:00:00Z"))
            .ok()
            .map(|value| value.with_timezone(&Utc));
    }
    let (month, day) = raw.split_once('/')?;
    let month: u32 = month.parse().ok()?;
    let day: u32 = day.parse().ok()?;
    let mut year = now.year();
    let mut candidate = chrono::NaiveDate::from_ymd_opt(year, month, day)?
        .and_hms_opt(0, 0, 0)?
        .and_utc();
    if candidate <= now {
        year += 1;
        candidate = chrono::NaiveDate::from_ymd_opt(year, month, day)?
            .and_hms_opt(0, 0, 0)?
            .and_utc();
    }
    Some(candidate)
}

/// Parses the Kiro CLI `/usage` panel into provider quota windows.
pub fn quota_from_usage_output(output: &str, now: DateTime<Utc>) -> Result<QuotaReport, String> {
    let text = strip_ansi(output);
    let lower = text.to_ascii_lowercase();
    if lower.contains("not logged in")
        || lower.contains("login required")
        || lower.contains("kiro-cli login")
        || lower.contains("oauth error")
    {
        return Err("AUTH_EXPIRED".to_string());
    }
    if lower.contains("could not retrieve usage information") {
        return Err("SOURCE_UNAVAILABLE".to_string());
    }

    let plan = text
        .lines()
        .find_map(|line| {
            line.split_once("Plan:")
                .map(|(_, value)| value.trim().to_string())
        })
        .filter(|value| !value.is_empty());
    let reset = parse_reset(&text, now);

    let mut used_percent = text
        .lines()
        .filter(|line| !line.to_ascii_lowercase().contains("bonus"))
        .find_map(|line| percent_before_marker(line, '%'));
    if used_percent.is_none() {
        if let Some((used, total)) = parse_covered(&text) {
            if total > 0.0 {
                used_percent = Some((used / total * 100.0).clamp(0.0, 100.0));
            }
        }
    }
    if used_percent.is_none()
        && (lower.contains("managed by admin") || lower.contains("managed by organization"))
    {
        used_percent = Some(0.0);
    }
    let Some(used_percent) = used_percent else {
        return Err("SOURCE_SCHEMA_MISMATCH".to_string());
    };

    let mut windows = vec![QuotaWindow::from_percent(
        "credits",
        "Credits",
        QuotaWindowScope::Other,
        QuotaKind::Credits,
        used_percent.clamp(0.0, 100.0),
        reset,
        Confidence::High,
    )];

    if let Some(bonus_start) = lower.find("bonus credits:") {
        let bonus = &text[bonus_start..];
        if let Some(slash) = bonus.find('/') {
            let before = &bonus[..slash];
            let after = &bonus[slash + 1..];
            let used_text = before
                .split_whitespace()
                .last()
                .unwrap_or_default()
                .trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
            let total_text = after
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(|ch: char| !ch.is_ascii_digit() && ch != '.');
            if let (Ok(used), Ok(total)) = (used_text.parse::<f64>(), total_text.parse::<f64>()) {
                if total > 0.0 {
                    windows.push(QuotaWindow::from_percent(
                        "bonus",
                        "Bonus credits",
                        QuotaWindowScope::Other,
                        QuotaKind::Credits,
                        (used / total * 100.0).clamp(0.0, 100.0),
                        None,
                        Confidence::High,
                    ));
                }
            }
        }
    }

    let mut report = QuotaReport::new(PROVIDER_ID, "provider_cli", windows, DEFAULT_FRESHNESS);
    report.plan = plan;
    Ok(report)
}

fn combined_output(output: &CommandOutput) -> String {
    [output.stdout.trim(), output.stderr.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub struct KiroAdapter {
    roots: Vec<PathBuf>,
    bounds: ScanBounds,
    binary: Option<PathBuf>,
}

impl Default for KiroAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiroAdapter {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let mut roots = vec![home.join(".kiro")];
        if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
            roots.push(appdata.join("Kiro").join("User").join("globalStorage"));
        }
        Self {
            roots,
            bounds: ScanBounds::default(),
            binary: resolve_binary("kiro-cli"),
        }
    }

    /// Adapter pinned to explicit source roots. The CLI is disabled so tests
    /// remain isolated from a developer's real Kiro installation.
    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            roots,
            bounds: ScanBounds::default(),
            binary: None,
        }
    }

    fn any_root_exists(&self) -> bool {
        self.roots.iter().any(|root| root.is_dir())
    }

    fn scan(&self) -> ScanReport {
        scan_directories(&self.roots, &self.bounds)
    }

    fn fetch_quota(&self) -> Result<Option<QuotaReport>, String> {
        let Some(binary) = &self.binary else {
            return Ok(None);
        };
        let version_output = run_command(binary, &["--version"], Duration::from_secs(2))?;
        if version_output.timed_out {
            return Err("PROVIDER_TIMEOUT".to_string());
        }
        let version = parse_cli_version(&combined_output(&version_output));
        if cfg!(windows) && version.is_some_and(requires_pty) {
            return Err("SOURCE_REQUIRES_TTY".to_string());
        }
        let output = run_command(binary, &["chat", "--no-interactive", "/usage"], CLI_TIMEOUT)?;
        if output.timed_out {
            return Err("PROVIDER_TIMEOUT".to_string());
        }
        if output.status != Some(0) && combined_output(&output).trim().is_empty() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        quota_from_usage_output(&combined_output(&output), Utc::now()).map(Some)
    }

    fn detection(&self) -> DetectionResult {
        let local = self.any_root_exists();
        let cli = self.binary.is_some();
        let source_exists = local || cli;
        let mut result = DetectionResult {
            provider_id: PROVIDER_ID.to_string(),
            display_name: "Kiro".to_string(),
            enabled: true,
            detected: source_exists,
            detection_method: if cli { "local_scan+cli" } else { "local_scan" }.to_string(),
            source_type: DATA_SOURCE.to_string(),
            source_exists,
            permission_state: "n/a".to_string(),
            adapter_version: ADAPTER_VERSION.to_string(),
            last_detection_at: Some(Utc::now().to_rfc3339()),
            detection_error_code: String::new(),
        };
        if !source_exists {
            result.permission_state = "not_found".to_string();
        } else if local && self.scan().is_empty() && !cli {
            result.detected = false;
            result.permission_state = "no_sessions".to_string();
        } else {
            result.permission_state = if cli { "read_ok+cli" } else { "read_ok" }.to_string();
        }
        result
    }
}

impl ProviderAdapter for KiroAdapter {
    fn descriptor(&self) -> AdapterDescriptor {
        AdapterDescriptor {
            id: PROVIDER_ID,
            display_name: "Kiro",
            vendor: "Kiro",
            source_kind: SourceKind::LocalJsonl,
            usage_support: ChannelSupport::LocalEstimate,
            quota_support: ChannelSupport::Native,
            auth: AuthKind::LocalFiles,
            adapter_version: ADAPTER_VERSION,
        }
    }

    fn collect_usage(&self) -> Result<UsageBatch, String> {
        if !self.any_root_exists() {
            return Err("SOURCE_UNAVAILABLE".to_string());
        }
        let report = self.scan();
        Ok(UsageBatch {
            batch_id: format!("{PROVIDER_ID}_{}", Utc::now().timestamp()),
            events: usage_events(
                PROVIDER_ID,
                DATA_SOURCE,
                &report.samples,
                Confidence::Medium,
            ),
        })
    }

    fn collect_quota(&self) -> Result<Option<QuotaReport>, String> {
        self.fetch_quota()
    }

    fn health_check(&self) -> AdapterHealth {
        let detection = self.detection();
        if !detection.source_exists {
            return AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Kiro local data and CLI not found".to_string(),
            };
        }
        if self.binary.is_some() {
            return match self.fetch_quota() {
                Ok(Some(_)) => AdapterHealth {
                    status: AdapterHealthStatus::Healthy,
                    message: "Kiro usage and quota available".to_string(),
                },
                Ok(None) => AdapterHealth {
                    status: AdapterHealthStatus::Degraded,
                    message: "Kiro quota unavailable".to_string(),
                },
                Err(code) => AdapterHealth {
                    status: AdapterHealthStatus::Degraded,
                    message: format!("Kiro quota unavailable ({code})"),
                },
            };
        }
        if detection.detected {
            AdapterHealth {
                status: AdapterHealthStatus::Healthy,
                message: "Kiro local records detected".to_string(),
            }
        } else {
            AdapterHealth {
                status: AdapterHealthStatus::Degraded,
                message: "Kiro local data has no token records".to_string(),
            }
        }
    }

    fn required_permissions(&self) -> Vec<Permission> {
        vec![Permission::FileSystem, Permission::Network]
    }

    fn detect(&self) -> Result<DetectionResult, String> {
        Ok(self.detection())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn recent_record(minutes_ago: i64, input: u64, output: u64) -> String {
        let ts = (Utc::now() - chrono::Duration::minutes(minutes_ago)).to_rfc3339();
        format!(
            r#"{{"timestamp":"{ts}","model":"kiro-default","usage":{{"input_tokens":{input},"output_tokens":{output}}}}}"#
        )
    }

    #[test]
    fn descriptor_declares_native_quota_support() {
        let adapter = KiroAdapter::with_roots(vec![PathBuf::from("Z:/missing")]);
        let descriptor = adapter.descriptor();
        descriptor.check().expect("descriptor is consistent");
        assert_eq!(descriptor.usage_support, ChannelSupport::LocalEstimate);
        assert_eq!(descriptor.quota_support, ChannelSupport::Native);
    }

    #[test]
    fn parser_reads_plan_percent_and_reset() {
        let output = "Plan: Kiro Pro\n████ 25%\nCredits (25 of 100 covered) resets on 2026-12-01";
        let report = quota_from_usage_output(output, Utc::now()).expect("quota");
        assert_eq!(report.plan.as_deref(), Some("Kiro Pro"));
        assert_eq!(report.windows[0].used_percent, Some(25.0));
        assert!(report.windows[0].reset_at.is_some());
    }

    #[test]
    fn parser_derives_percent_from_covered_counts() {
        let output = "Plan: Kiro Free\nCredits (20 of 80 covered) resets on 12/31";
        let report = quota_from_usage_output(output, Utc::now()).expect("quota");
        assert_eq!(report.windows[0].used_percent, Some(25.0));
    }

    #[test]
    fn new_cli_versions_are_recognized_as_pty_only() {
        let version = parse_cli_version("kiro-cli 2.13.1").expect("version");
        assert!(requires_pty(version));
        assert!(!requires_pty(CliVersion {
            major: 2,
            minor: 12,
            patch: 9
        }));
    }

    #[test]
    fn missing_source_reports_an_error_instead_of_empty_success() {
        let adapter = KiroAdapter::with_roots(vec![PathBuf::from("Z:/definitely/missing")]);
        assert_eq!(
            adapter
                .collect_usage()
                .expect_err("missing source must fail"),
            "SOURCE_UNAVAILABLE"
        );
        assert!(adapter.collect_quota().expect("quota call").is_none());
    }

    #[test]
    fn collects_real_records_from_local_files() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("data");
        std::fs::create_dir_all(root.join("sessions")).expect("create dirs");
        std::fs::write(
            root.join("sessions").join("history.jsonl"),
            format!(
                "{}\n{}",
                recent_record(5, 300, 100),
                recent_record(20, 10, 5)
            ),
        )
        .expect("write history");

        let adapter = KiroAdapter::with_roots(vec![root]);
        let batch = adapter.collect_usage().expect("usage");
        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].provider_id, PROVIDER_ID);
        assert_eq!(batch.events[0].model, "kiro-default");
        assert_eq!(adapter.health_check().status, AdapterHealthStatus::Healthy);
    }
}
