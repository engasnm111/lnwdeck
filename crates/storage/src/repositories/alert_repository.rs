//! Alert persistence.
//!
//! Alerts are raised only by the evaluator from real quota, collector and
//! budget state. Repeated occurrences of the same condition update one row
//! instead of piling up, and an alert whose condition disappears is resolved
//! rather than deleted, so the history stays honest.

use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    QuotaThreshold,
    CollectorError,
    AuthExpired,
    RateLimited,
    BudgetWarning,
    BudgetExceeded,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::QuotaThreshold => "quota_threshold",
            Self::CollectorError => "collector_error",
            Self::AuthExpired => "auth_expired",
            Self::RateLimited => "rate_limited",
            Self::BudgetWarning => "budget_warning",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "quota_threshold" => Some(Self::QuotaThreshold),
            "collector_error" => Some(Self::CollectorError),
            "auth_expired" => Some(Self::AuthExpired),
            "rate_limited" => Some(Self::RateLimited),
            "budget_warning" => Some(Self::BudgetWarning),
            "budget_exceeded" => Some(Self::BudgetExceeded),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

impl AlertSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "info" => Some(Self::Info),
            "warning" => Some(Self::Warning),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// One alert row. `alert_key` is the dedupe key: the same condition always
/// produces the same key.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AlertRow {
    pub id: i64,
    pub alert_key: String,
    pub kind: AlertKind,
    pub severity: AlertSeverity,
    pub provider_id: String,
    pub title: String,
    pub detail: String,
    pub error_code: String,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub occurrences: i64,
    pub acknowledged_at: Option<String>,
    pub resolved_at: Option<String>,
}

/// A condition the evaluator observed right now.
#[derive(Debug, Clone, PartialEq)]
pub struct AlertObservation {
    pub alert_key: String,
    pub kind: AlertKind,
    pub severity: AlertSeverity,
    pub provider_id: String,
    pub title: String,
    pub detail: String,
    pub error_code: String,
}

pub struct AlertRepository<'a> {
    conn: &'a Connection,
}

impl<'a> AlertRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Records an observation.
    ///
    /// A new condition inserts a row; a recurring one bumps `last_seen_at` and
    /// the occurrence count. A previously resolved condition that reappears is
    /// re-opened and, because it is a new occurrence, its acknowledgement is
    /// cleared.
    pub fn observe(&self, observation: &AlertObservation) -> Result<i64, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let existing: Option<(i64, Option<String>)> = self
            .conn
            .query_row(
                "SELECT id, resolved_at FROM alerts WHERE alert_key = ?1",
                [&observation.alert_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        match existing {
            Some((id, resolved_at)) => {
                let was_resolved = resolved_at.is_some();
                self.conn.execute(
                    "UPDATE alerts SET
                        kind = ?2, severity = ?3, provider_id = ?4, title = ?5, detail = ?6,
                        error_code = ?7, last_seen_at = ?8, occurrences = occurrences + 1,
                        resolved_at = NULL,
                        acknowledged_at = CASE WHEN ?9 = 1 THEN NULL ELSE acknowledged_at END
                     WHERE id = ?1",
                    params![
                        id,
                        observation.kind.as_str(),
                        observation.severity.as_str(),
                        observation.provider_id,
                        observation.title,
                        observation.detail,
                        observation.error_code,
                        now,
                        was_resolved as i64,
                    ],
                )?;
                Ok(id)
            }
            None => {
                self.conn.execute(
                    "INSERT INTO alerts
                        (alert_key, kind, severity, provider_id, title, detail, error_code,
                         first_seen_at, last_seen_at, occurrences)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, 1)",
                    params![
                        observation.alert_key,
                        observation.kind.as_str(),
                        observation.severity.as_str(),
                        observation.provider_id,
                        observation.title,
                        observation.detail,
                        observation.error_code,
                        now,
                    ],
                )?;
                Ok(self.conn.last_insert_rowid())
            }
        }
    }

    /// Marks every open alert whose key is not in `active_keys` as resolved.
    /// Returns the number of alerts closed.
    pub fn resolve_missing(&self, active_keys: &[String]) -> Result<usize, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        if active_keys.is_empty() {
            return self.conn.execute(
                "UPDATE alerts SET resolved_at = ?1 WHERE resolved_at IS NULL",
                [&now],
            );
        }
        let placeholders = std::iter::repeat_n("?", active_keys.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "UPDATE alerts SET resolved_at = ?1
             WHERE resolved_at IS NULL AND alert_key NOT IN ({placeholders})"
        );
        let mut values: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(active_keys.len() + 1);
        values.push(&now);
        for key in active_keys {
            values.push(key);
        }
        self.conn.execute(&sql, values.as_slice())
    }

    /// Open alerts, most recently seen first.
    pub fn open_alerts(&self) -> Result<Vec<AlertRow>, rusqlite::Error> {
        self.query("WHERE resolved_at IS NULL ORDER BY last_seen_at DESC, id DESC")
    }

    /// Every alert including resolved ones, newest first, capped.
    pub fn history(&self, limit: usize) -> Result<Vec<AlertRow>, rusqlite::Error> {
        self.query(&format!(
            "ORDER BY last_seen_at DESC, id DESC LIMIT {}",
            limit.clamp(1, 500)
        ))
    }

    /// Acknowledges one alert. Returns false when the id is unknown.
    pub fn acknowledge(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let updated = self.conn.execute(
            "UPDATE alerts SET acknowledged_at = ?2 WHERE id = ?1 AND acknowledged_at IS NULL",
            params![id, now],
        )?;
        Ok(updated > 0)
    }

    /// Acknowledges every currently open, unread alert atomically.
    pub fn acknowledge_all_open(&self) -> Result<usize, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let transaction = self.conn.unchecked_transaction()?;
        let updated = transaction.execute(
            "UPDATE alerts
             SET acknowledged_at = ?1
             WHERE resolved_at IS NULL AND acknowledged_at IS NULL",
            [&now],
        )?;
        transaction.commit()?;
        Ok(updated)
    }

    /// Deletes resolved alerts older than `before`. Returns rows removed.
    pub fn prune_resolved(&self, before: &str) -> Result<usize, rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM alerts WHERE resolved_at IS NOT NULL AND resolved_at < ?1",
            [before],
        )
    }

    fn query(&self, suffix: &str) -> Result<Vec<AlertRow>, rusqlite::Error> {
        let sql = format!(
            "SELECT id, alert_key, kind, severity, provider_id, title, detail, error_code,
                    first_seen_at, last_seen_at, occurrences, acknowledged_at, resolved_at
             FROM alerts {suffix}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            let kind: String = row.get(2)?;
            let severity: String = row.get(3)?;
            Ok(AlertRow {
                id: row.get(0)?,
                alert_key: row.get(1)?,
                kind: AlertKind::parse(&kind).unwrap_or(AlertKind::CollectorError),
                severity: AlertSeverity::parse(&severity).unwrap_or(AlertSeverity::Warning),
                provider_id: row.get(4)?,
                title: row.get(5)?,
                detail: row.get(6)?,
                error_code: row.get(7)?,
                first_seen_at: row.get(8)?,
                last_seen_at: row.get(9)?,
                occurrences: row.get(10)?,
                acknowledged_at: row.get(11)?,
                resolved_at: row.get(12)?,
            })
        })?;
        let mut alerts = Vec::new();
        for row in rows {
            alerts.push(row?);
        }
        Ok(alerts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("test.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    fn observation(key: &str) -> AlertObservation {
        AlertObservation {
            alert_key: key.to_string(),
            kind: AlertKind::CollectorError,
            severity: AlertSeverity::Warning,
            provider_id: "opencode".to_string(),
            title: "OpenCode collection failed".to_string(),
            detail: "the local store could not be read".to_string(),
            error_code: "SOURCE_UNAVAILABLE".to_string(),
        }
    }

    #[test]
    fn table_starts_empty() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        assert!(repo.open_alerts().expect("open").is_empty());
        assert!(repo.history(50).expect("history").is_empty());
    }

    #[test]
    fn repeated_observations_update_one_row() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        let first = repo
            .observe(&observation("collector:opencode"))
            .expect("first");
        let second = repo
            .observe(&observation("collector:opencode"))
            .expect("second");
        assert_eq!(first, second);

        let open = repo.open_alerts().expect("open");
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].occurrences, 2);
        assert_eq!(open[0].error_code, "SOURCE_UNAVAILABLE");
        assert!(open[0].resolved_at.is_none());
    }

    #[test]
    fn conditions_that_disappear_are_resolved_not_deleted() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        repo.observe(&observation("collector:opencode")).expect("a");
        repo.observe(&observation("collector:codex")).expect("b");

        let closed = repo
            .resolve_missing(&["collector:opencode".to_string()])
            .expect("resolve");
        assert_eq!(closed, 1);

        let open = repo.open_alerts().expect("open");
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].alert_key, "collector:opencode");

        let history = repo.history(50).expect("history");
        assert_eq!(history.len(), 2, "resolved alerts stay in the history");
        assert!(history
            .iter()
            .any(|alert| alert.alert_key == "collector:codex" && alert.resolved_at.is_some()));
    }

    #[test]
    fn resolving_with_no_active_keys_closes_everything() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        repo.observe(&observation("a")).expect("a");
        repo.observe(&observation("b")).expect("b");
        assert_eq!(repo.resolve_missing(&[]).expect("resolve"), 2);
        assert!(repo.open_alerts().expect("open").is_empty());
    }

    #[test]
    fn acknowledgement_is_recorded_once_and_cleared_on_recurrence() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        let id = repo.observe(&observation("collector:opencode")).expect("a");

        assert!(repo.acknowledge(id).expect("acknowledge"));
        assert!(
            !repo.acknowledge(id).expect("second acknowledge"),
            "an already acknowledged alert reports no change"
        );
        assert!(repo.open_alerts().expect("open")[0]
            .acknowledged_at
            .is_some());

        // Still failing: acknowledgement survives while the alert stays open.
        repo.observe(&observation("collector:opencode")).expect("b");
        assert!(repo.open_alerts().expect("open")[0]
            .acknowledged_at
            .is_some());

        // Resolved, then failing again: the user must see it once more.
        repo.resolve_missing(&[]).expect("resolve");
        repo.observe(&observation("collector:opencode")).expect("c");
        let open = repo.open_alerts().expect("open");
        assert_eq!(open.len(), 1);
        assert!(
            open[0].acknowledged_at.is_none(),
            "a condition that comes back must be shown again"
        );
        assert!(open[0].resolved_at.is_none());
    }

    #[test]
    fn acknowledging_an_unknown_id_reports_no_change() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        assert!(!repo.acknowledge(999).expect("acknowledge"));
    }

    #[test]
    fn acknowledge_all_marks_only_open_unacknowledged_alerts_in_one_transaction() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        let acknowledged = repo.observe(&observation("already-seen")).expect("observe");
        repo.acknowledge(acknowledged).expect("acknowledge");
        repo.observe(&observation("needs-reading"))
            .expect("observe");
        repo.observe(&observation("will-resolve")).expect("observe");
        repo.resolve_missing(&["already-seen".to_string(), "needs-reading".to_string()])
            .expect("resolve");

        assert_eq!(repo.acknowledge_all_open().expect("mark all"), 1);
        let open = repo.open_alerts().expect("open");
        assert!(open.iter().all(|alert| alert.acknowledged_at.is_some()));
        assert!(open
            .iter()
            .find(|alert| alert.alert_key == "already-seen")
            .and_then(|alert| alert.acknowledged_at.as_ref())
            .is_some());
    }

    #[test]
    fn prune_removes_only_old_resolved_alerts() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        repo.observe(&observation("old")).expect("old");
        repo.resolve_missing(&[]).expect("resolve");
        repo.observe(&observation("current")).expect("current");

        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        assert_eq!(repo.prune_resolved(&future).expect("prune"), 1);
        let remaining = repo.history(50).expect("history");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].alert_key, "current");
    }

    #[test]
    fn kinds_and_severities_roundtrip() {
        let storage = open_db();
        let repo = AlertRepository::new(&storage.conn);
        repo.observe(&AlertObservation {
            kind: AlertKind::BudgetExceeded,
            severity: AlertSeverity::Critical,
            ..observation("budget:global:monthly")
        })
        .expect("observe");
        let stored = &repo.open_alerts().expect("open")[0];
        assert_eq!(stored.kind, AlertKind::BudgetExceeded);
        assert_eq!(stored.severity, AlertSeverity::Critical);
    }
}
