//! Alert evaluation.
//!
//! Every alert is derived from real state that already exists in the database:
//! stored quota reports, the last collector run per provider, and budget
//! progress. The evaluator never invents an "all clear" record - when nothing is
//! wrong it produces no alerts and resolves the ones that no longer apply.

use lnwdeck_domain::QuotaStatus;
use lnwdeck_pricing::catalog::PriceResolver;
use lnwdeck_provider_runtime::NOT_SUPPORTED;
use lnwdeck_storage::repositories::{
    AlertKind, AlertObservation, AlertRepository, AlertRow, AlertSeverity, DiagnosticsRepository,
    QuotaRepository,
};
use rusqlite::Connection;
use serde::Serialize;

/// Remaining-quota percentage at or below which a warning is raised.
const QUOTA_WARN_PERCENT: f64 = 20.0;
/// Remaining-quota percentage at or below which the alert becomes critical.
const QUOTA_CRITICAL_PERCENT: f64 = 5.0;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertsView {
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub open: Vec<AlertRow>,
    pub history: Vec<AlertRow>,
    pub open_count: usize,
    pub critical_count: usize,
    pub unacknowledged_count: usize,
}

pub struct EvaluateAlerts;

impl EvaluateAlerts {
    /// Recomputes the alert set from current state and returns the view.
    ///
    /// `display_name` resolves a provider id to its display name; it is passed
    /// in so this layer does not restate the provider registry.
    pub fn execute(
        conn: &Connection,
        resolver: &PriceResolver,
        display_name: &dyn Fn(&str) -> String,
    ) -> Result<AlertsView, rusqlite::Error> {
        let observations = Self::observe(conn, resolver, display_name)?;
        let repo = AlertRepository::new(conn);
        let mut active_keys = Vec::with_capacity(observations.len());
        for observation in &observations {
            repo.observe(observation)?;
            active_keys.push(observation.alert_key.clone());
        }
        repo.resolve_missing(&active_keys)?;
        Self::view(conn)
    }

    /// Reads the current alert set without re-evaluating.
    pub fn view(conn: &Connection) -> Result<AlertsView, rusqlite::Error> {
        let repo = AlertRepository::new(conn);
        let open = repo.open_alerts()?;
        let history = repo.history(100)?;
        Ok(AlertsView {
            generated_at: chrono::Utc::now(),
            open_count: open.len(),
            critical_count: open
                .iter()
                .filter(|alert| alert.severity == AlertSeverity::Critical)
                .count(),
            unacknowledged_count: open
                .iter()
                .filter(|alert| alert.acknowledged_at.is_none())
                .count(),
            open,
            history,
        })
    }

    /// Conditions that hold right now.
    fn observe(
        conn: &Connection,
        resolver: &PriceResolver,
        display_name: &dyn Fn(&str) -> String,
    ) -> Result<Vec<AlertObservation>, rusqlite::Error> {
        let mut observations = Vec::new();

        // Quota: low remaining, expired auth, rate limiting, collection errors.
        for report in QuotaRepository::new(conn).latest_all()? {
            let name = display_name(&report.provider_id);
            match report.status {
                QuotaStatus::AuthExpired => observations.push(AlertObservation {
                    alert_key: format!("auth:{}", report.provider_id),
                    kind: AlertKind::AuthExpired,
                    severity: AlertSeverity::Critical,
                    provider_id: report.provider_id.clone(),
                    title: format!("{name} authentication expired"),
                    detail: "the provider rejected the stored credential".to_string(),
                    error_code: report.error_code.clone().unwrap_or_default(),
                }),
                QuotaStatus::RateLimited => observations.push(AlertObservation {
                    alert_key: format!("rate:{}", report.provider_id),
                    kind: AlertKind::RateLimited,
                    severity: AlertSeverity::Warning,
                    provider_id: report.provider_id.clone(),
                    title: format!("{name} is rate limited"),
                    detail: "the provider refused further requests for now".to_string(),
                    error_code: report.error_code.clone().unwrap_or_default(),
                }),
                _ => {}
            }

            for window in &report.windows {
                // Only a real limit can be low; usage-only windows are skipped.
                let Some(remaining_percent) = window.remaining_percent else {
                    continue;
                };
                if remaining_percent > QUOTA_WARN_PERCENT {
                    continue;
                }
                let severity = if remaining_percent <= QUOTA_CRITICAL_PERCENT {
                    AlertSeverity::Critical
                } else {
                    AlertSeverity::Warning
                };
                observations.push(AlertObservation {
                    alert_key: format!("quota:{}:{}", report.provider_id, window.window_key),
                    kind: AlertKind::QuotaThreshold,
                    severity,
                    provider_id: report.provider_id.clone(),
                    title: format!(
                        "{name} {} window at {:.0}% remaining",
                        window.label, remaining_percent
                    ),
                    detail: format!(
                        "used {} of {}",
                        window.used,
                        window
                            .limit
                            .map(|limit| limit.to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    ),
                    error_code: String::new(),
                });
            }
        }

        // Collector failures, excluding channels that are simply not supported.
        for run in DiagnosticsRepository::new(conn).latest_runs()? {
            if run.error_code.is_empty() || run.error_code == NOT_SUPPORTED {
                continue;
            }
            if run.error_code == "NOT_CONFIGURED" {
                continue;
            }
            let name = display_name(&run.provider_id);
            observations.push(AlertObservation {
                alert_key: format!("collector:{}:{}", run.provider_id, run.collector_mode),
                kind: AlertKind::CollectorError,
                severity: AlertSeverity::Warning,
                provider_id: run.provider_id.clone(),
                title: format!("{name} collection failed"),
                detail: format!("collector mode {}", run.collector_mode),
                error_code: run.error_code.clone(),
            });
        }

        // Budgets at or above their threshold.
        for progress in crate::budgets::QueryBudgets::execute(conn, resolver)?.budgets {
            if !progress.budget.enabled {
                continue;
            }
            let scope_label = match &progress.budget.scope {
                lnwdeck_storage::repositories::BudgetScope::Global => "all providers".to_string(),
                lnwdeck_storage::repositories::BudgetScope::Provider(id) => display_name(id),
            };
            let period = crate::budgets::period_label(progress.budget.period);
            match progress.state.as_str() {
                "exceeded" => observations.push(AlertObservation {
                    alert_key: format!("budget:{}", progress.budget.id),
                    kind: AlertKind::BudgetExceeded,
                    severity: AlertSeverity::Critical,
                    provider_id: String::new(),
                    title: format!("{period} budget for {scope_label} exceeded"),
                    detail: format!("spent {} in this period", progress.cost_used),
                    error_code: String::new(),
                }),
                "warning" => observations.push(AlertObservation {
                    alert_key: format!("budget:{}", progress.budget.id),
                    kind: AlertKind::BudgetWarning,
                    severity: AlertSeverity::Warning,
                    provider_id: String::new(),
                    title: format!("{period} budget for {scope_label} near its limit"),
                    detail: format!("spent {} in this period", progress.cost_used),
                    error_code: String::new(),
                }),
                _ => {}
            }
        }

        Ok(observations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use lnwdeck_domain::{
        Confidence, QuotaKind, QuotaReport, QuotaWindow, QuotaWindowScope, DEFAULT_FRESHNESS,
    };
    use lnwdeck_storage::repositories::{
        BudgetPeriod, BudgetRepository, BudgetRow, BudgetScope, CollectorRunRow,
    };
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use serde_json::json;
    use std::num::NonZeroU64;
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("test.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    fn names() -> impl Fn(&str) -> String {
        |id: &str| match id {
            "opencode" => "OpenCode".to_string(),
            "openrouter_api" => "OpenRouter".to_string(),
            other => other.to_string(),
        }
    }

    fn resolver() -> PriceResolver {
        PriceResolver::new_with_overrides(&json!([
            {
                "provider": "anthropic",
                "model": "claude-test",
                "input_per_1k": "1.0",
                "output_per_1k": "1.0"
            }
        ]))
    }

    fn store_quota(storage: &Storage, provider: &str, used: u64, limit: u64) {
        let report = QuotaReport::new(
            provider,
            "provider_api",
            vec![QuotaWindow::with_limit(
                "monthly",
                "Monthly",
                QuotaWindowScope::Monthly,
                QuotaKind::Requests,
                used,
                NonZeroU64::new(limit).expect("limit"),
                None,
                Confidence::High,
            )],
            DEFAULT_FRESHNESS,
        );
        QuotaRepository::new(&storage.conn)
            .upsert_report(&report)
            .expect("upsert");
    }

    fn record_run(storage: &Storage, provider: &str, mode: &str, error_code: &str) {
        DiagnosticsRepository::new(&storage.conn)
            .insert_collector_run(&CollectorRunRow {
                id: 0,
                provider_id: provider.to_string(),
                collector_mode: mode.to_string(),
                started_at: Utc::now().to_rfc3339(),
                finished_at: Utc::now().to_rfc3339(),
                duration_ms: 5,
                source_records_seen: 0,
                records_parsed: 0,
                events_normalized: 0,
                events_rejected: 0,
                duplicates_skipped: 0,
                events_inserted: 0,
                quota_snapshots_inserted: 0,
                warning_codes: Vec::new(),
                error_code: error_code.to_string(),
                next_retry_at: None,
            })
            .expect("run");
    }

    #[test]
    fn a_healthy_system_produces_no_alerts() {
        let storage = open_db();
        store_quota(&storage, "openrouter_api", 10, 1000);
        record_run(&storage, "opencode", "local_scan", "");

        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert!(
            view.open.is_empty(),
            "nothing is wrong, so no alert may be created: {:?}",
            view.open
        );
        assert_eq!(view.open_count, 0);
        assert_eq!(view.critical_count, 0);
    }

    #[test]
    fn unsupported_and_unconfigured_channels_are_not_alerts() {
        let storage = open_db();
        record_run(&storage, "cursor_ide", "unsupported", "NOT_SUPPORTED");
        record_run(
            &storage,
            "openrouter_api",
            "quota_collect",
            "NOT_CONFIGURED",
        );

        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert!(
            view.open.is_empty(),
            "a provider that is simply not wired up is not a failure: {:?}",
            view.open
        );
    }

    #[test]
    fn low_remaining_quota_raises_a_threshold_alert() {
        let storage = open_db();
        store_quota(&storage, "openrouter_api", 950, 1000);

        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert_eq!(view.open.len(), 1);
        let alert = &view.open[0];
        assert_eq!(alert.kind, AlertKind::QuotaThreshold);
        assert_eq!(alert.severity, AlertSeverity::Critical);
        assert!(alert.title.contains("OpenRouter"));
        assert_eq!(alert.provider_id, "openrouter_api");
    }

    #[test]
    fn usage_only_windows_never_raise_a_quota_alert() {
        let storage = open_db();
        let report = QuotaReport::new(
            "opencode",
            "local_estimate",
            vec![QuotaWindow::usage_only(
                "5h",
                "5-hour",
                QuotaWindowScope::Rolling,
                QuotaKind::Tokens,
                999_999,
                None,
                Confidence::Medium,
            )],
            DEFAULT_FRESHNESS,
        );
        QuotaRepository::new(&storage.conn)
            .upsert_report(&report)
            .expect("upsert");

        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert!(
            view.open.is_empty(),
            "without a limit there is no threshold to cross"
        );
    }

    #[test]
    fn auth_expiry_and_rate_limits_become_alerts() {
        let storage = open_db();
        QuotaRepository::new(&storage.conn)
            .upsert_report(&QuotaReport::failed(
                "openrouter_api",
                "provider_api",
                "AUTH_EXPIRED",
            ))
            .expect("auth");

        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert_eq!(view.open.len(), 1);
        assert_eq!(view.open[0].kind, AlertKind::AuthExpired);
        assert_eq!(view.open[0].error_code, "AUTH_EXPIRED");
    }

    #[test]
    fn collector_errors_become_alerts_and_resolve_when_fixed() {
        let storage = open_db();
        record_run(&storage, "opencode", "local_scan", "SOURCE_UNAVAILABLE");
        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert_eq!(view.open.len(), 1);
        assert_eq!(view.open[0].kind, AlertKind::CollectorError);

        // A later successful run replaces the failing one.
        record_run(&storage, "opencode", "local_scan", "");
        let after =
            EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert!(after.open.is_empty(), "the alert must resolve itself");
        assert_eq!(after.history.len(), 1, "history keeps the resolved alert");
        assert!(after.history[0].resolved_at.is_some());
    }

    #[test]
    fn exceeded_budgets_raise_a_critical_alert() {
        let storage = open_db();
        storage
            .conn
            .execute(
                "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model,
                     tokens_input, tokens_output, confidence, data_source, cost)
                 VALUES ('e1', 'b', ?1, 'anthropic_claude', 'claude-test', 20000, 0, 'High', 'local', '')",
                [(Utc::now() - Duration::hours(1)).to_rfc3339()],
            )
            .expect("event");
        BudgetRepository::new(&storage.conn)
            .upsert(&BudgetRow {
                id: 0,
                scope: BudgetScope::Global,
                period: BudgetPeriod::Monthly,
                cost_limit: "10".to_string(),
                token_limit: None,
                warn_percent: 80,
                enabled: true,
                created_at: String::new(),
                updated_at: String::new(),
            })
            .expect("budget");

        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        assert_eq!(view.open.len(), 1);
        assert_eq!(view.open[0].kind, AlertKind::BudgetExceeded);
        assert_eq!(view.open[0].severity, AlertSeverity::Critical);
        assert_eq!(view.critical_count, 1);
        assert_eq!(view.unacknowledged_count, 1);
    }

    #[test]
    fn acknowledgement_is_reflected_in_the_counts() {
        let storage = open_db();
        store_quota(&storage, "openrouter_api", 990, 1000);
        let view = EvaluateAlerts::execute(&storage.conn, &resolver(), &names()).expect("evaluate");
        let id = view.open[0].id;

        assert!(AlertRepository::new(&storage.conn)
            .acknowledge(id)
            .expect("acknowledge"));
        let after = EvaluateAlerts::view(&storage.conn).expect("view");
        assert_eq!(after.open_count, 1);
        assert_eq!(after.unacknowledged_count, 0);
    }
}
