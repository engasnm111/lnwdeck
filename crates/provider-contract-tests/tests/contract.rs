//! Contract suite shared by every built-in provider adapter.
//!
//! These tests codify the standardized-data invariants that the quota core,
//! dashboard, and widget rely on: stable identifiers, graceful detection and
//! collection (never panics), and — whenever a quota report is produced —
//! privacy-safe payloads with no fabricated percentages.

use lnwdeck_provider_claude::ClaudeAdapter;
use lnwdeck_provider_codex::CodexAdapter;
use lnwdeck_provider_copilot::CopilotAdapter;
use lnwdeck_provider_cursor::CursorAdapter;
use lnwdeck_provider_gemini::GeminiAdapter;
use lnwdeck_provider_grok::GrokAdapter;
use lnwdeck_provider_kiro::KiroAdapter;
use lnwdeck_provider_ollama::OllamaAdapter;
use lnwdeck_provider_opencode::OpenCodeAdapter;
use lnwdeck_provider_openrouter::OpenRouterAdapter;
use lnwdeck_provider_runtime::ProviderAdapter;
use lnwdeck_security::PrivacyGuard;
use std::collections::HashSet;

fn builtin_adapters() -> Vec<Box<dyn ProviderAdapter>> {
    vec![
        Box::new(ClaudeAdapter::new()),
        Box::new(CodexAdapter::new()),
        Box::new(CopilotAdapter),
        Box::new(CursorAdapter),
        Box::new(GeminiAdapter),
        Box::new(GrokAdapter),
        Box::new(KiroAdapter),
        Box::new(OllamaAdapter::new()),
        Box::new(OpenCodeAdapter::new(&[0u8; 32])),
        Box::new(OpenRouterAdapter),
    ]
}

#[test]
fn every_adapter_has_a_stable_unique_identifier() {
    let adapters = builtin_adapters();
    let mut ids = HashSet::new();
    for adapter in &adapters {
        let id = adapter.id();
        assert!(!id.is_empty(), "provider id must not be empty");
        assert!(
            ids.insert(id),
            "provider ids must be unique: duplicate {id}"
        );
    }
    assert_eq!(adapters.len(), 10, "all built-in providers registered");
}

#[test]
fn every_adapter_has_a_display_name() {
    for adapter in builtin_adapters() {
        assert!(
            !adapter.name().is_empty(),
            "provider {} must have a display name",
            adapter.id()
        );
    }
}

#[test]
fn detection_never_panics_and_is_sanitized() {
    for adapter in builtin_adapters() {
        let result = adapter.detect().expect("detect must not fail");
        assert_eq!(result.provider_id, adapter.id(), "detection provider id");
        assert_eq!(
            result.display_name,
            adapter.name(),
            "detection display name"
        );
        assert!(
            !result.detection_error_code.contains('\\')
                && !result.detection_error_code.contains("Bearer"),
            "detection error codes are sanitized"
        );
        assert!(
            !result.source_type.contains('\\'),
            "source_type carries no paths"
        );
    }
}

#[test]
fn quota_collection_never_panics_and_reports_canonical_id() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        assert_eq!(
            result.outcome.provider_id,
            adapter.id(),
            "quota outcome provider id for {}",
            adapter.id()
        );
        assert!(
            !result.outcome.error_code.contains('\\'),
            "quota error codes are sanitized"
        );
    }
}

#[test]
fn quota_reports_are_privacy_safe() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if let Some(report) = result.report {
            assert_eq!(
                report.provider_id,
                adapter.id(),
                "quota report must use the canonical provider id"
            );
            assert!(
                PrivacyGuard::validate_quota_report(&report).is_ok(),
                "quota report for {} must pass the privacy guard",
                adapter.id()
            );
        }
    }
}

#[test]
fn quota_reports_never_fabricate_percentages() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if let Some(report) = result.report {
            for window in &report.windows {
                if window.limit == 0 && !window.is_unlimited {
                    assert_eq!(
                        window.used_percent,
                        0.0,
                        "{} window {} has unknown limit; used_percent must be 0, not fabricated",
                        adapter.id(),
                        window.window_key
                    );
                }
                if window.is_unlimited {
                    assert_eq!(window.limit, 0, "unlimited window has no limit");
                    assert_eq!(window.used_percent, 0.0);
                }
                assert!(
                    (0.0..=100.0).contains(&window.remaining_percent),
                    "remaining_percent in range"
                );
            }
        }
    }
}

#[test]
fn error_quota_reports_carry_no_windows_or_data() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if result.outcome.status.is_error() {
            assert!(
                result.report.is_none()
                    || result.report.as_ref().is_some_and(|r| r.windows.is_empty()),
                "{} error report must not fabricate windows",
                adapter.id()
            );
            assert!(
                !result.outcome.error_code.is_empty(),
                "{} error status requires an error code",
                adapter.id()
            );
        }
    }
}

#[test]
fn success_outcomes_carry_a_usable_report() {
    for adapter in builtin_adapters() {
        let result = adapter.collect_quota_report();
        if !result.outcome.status.is_error() && result.outcome.windows_collected > 0 {
            let report = result
                .report
                .expect("windows collected implies a report is present");
            assert!(report.is_usable(), "{} report is usable", adapter.id());
        }
    }
}
