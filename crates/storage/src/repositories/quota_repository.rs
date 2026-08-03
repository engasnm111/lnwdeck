use chrono::{DateTime, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaStatus, QuotaWindow, QuotaWindowScope,
};
use rusqlite::{params, Connection, OptionalExtension};

/// Result of an `upsert_report` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaUpsert {
    Inserted,
    Replaced,
}

/// One historical window snapshot returned by `history`.
#[derive(Debug, Clone, PartialEq)]
pub struct QuotaHistoryEntry {
    pub collected_at: DateTime<Utc>,
    pub window: QuotaWindow,
}

/// Persistence for normalized quota reports and their windows.
///
/// The latest report per provider lives in `quota_reports`; every window
/// snapshot is appended to `quota_windows` so callers can query history and
/// prune old data without losing the current state.
pub struct QuotaRepository<'a> {
    conn: &'a Connection,
}

impl<'a> QuotaRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Stores or replaces the latest report for a provider. Older reports
    /// never clobber newer ones: when the incoming report is older than the
    /// stored one, it is ignored and `Replaced` is returned.
    pub fn upsert_report(&self, report: &QuotaReport) -> Result<QuotaUpsert, rusqlite::Error> {
        let collected_at = report.collected_at.to_rfc3339();
        let stored: Option<String> = self
            .conn
            .query_row(
                "SELECT collected_at FROM quota_reports WHERE provider_id = ?1",
                [&report.provider_id],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(stored_at) = stored {
            let stored_dt = DateTime::parse_from_rfc3339(&stored_at)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or(Utc::now());
            if report.collected_at < stored_dt {
                return Ok(QuotaUpsert::Replaced);
            }
        }

        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO quota_reports
                (provider_id, account_fingerprint, plan, status, source, collected_at, stale_at, error_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                report.provider_id,
                report.account_fingerprint.as_deref().unwrap_or(""),
                report.plan.as_deref().unwrap_or(""),
                status_to_str(report.status),
                report.source,
                collected_at,
                report.stale_at.to_rfc3339(),
                report.error_code.as_deref(),
            ],
        )?;

        tx.execute(
            "DELETE FROM quota_windows WHERE provider_id = ?1 AND collected_at = ?2",
            params![report.provider_id, collected_at],
        )?;

        for window in &report.windows {
            tx.execute(
                "INSERT INTO quota_windows
                    (provider_id, window_key, label, scope, kind, used, quota_limit, remaining,
                     used_percent, remaining_percent, reset_at, is_unlimited, confidence, collected_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    report.provider_id,
                    window.window_key,
                    window.label,
                    scope_to_str(window.scope),
                    kind_to_str(window.kind),
                    window.used as i64,
                    window.limit as i64,
                    window.remaining as i64,
                    window.used_percent,
                    window.remaining_percent,
                    window.reset_at.map(|dt| dt.to_rfc3339()),
                    window.is_unlimited as i64,
                    confidence_to_str(window.confidence),
                    collected_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(QuotaUpsert::Inserted)
    }

    /// Latest stored report for one provider, if any.
    pub fn latest_report(&self, provider_id: &str) -> Result<Option<QuotaReport>, rusqlite::Error> {
        let row = self
            .conn
            .query_row(
                "SELECT provider_id, account_fingerprint, plan, status, source,
                        collected_at, stale_at, error_code
                 FROM quota_reports WHERE provider_id = ?1",
                [provider_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                    ))
                },
            )
            .optional()?;

        let Some((
            provider_id,
            fingerprint,
            plan,
            status,
            source,
            collected_at,
            stale_at,
            error_code,
        )) = row
        else {
            return Ok(None);
        };

        let collected_dt = parse_rfc3339(&collected_at);
        let windows = self.windows_for(&provider_id, &collected_at)?;

        Ok(Some(QuotaReport {
            provider_id,
            account_fingerprint: (!fingerprint.is_empty()).then_some(fingerprint),
            plan: (!plan.is_empty()).then_some(plan),
            status: parse_status(&status),
            source,
            collected_at: collected_dt,
            stale_at: parse_rfc3339(&stale_at),
            error_code,
            windows,
        }))
    }

    /// Latest report for every provider that has one.
    pub fn latest_all(&self) -> Result<Vec<QuotaReport>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT provider_id, account_fingerprint, plan, status, source,
                    collected_at, stale_at, error_code
             FROM quota_reports ORDER BY provider_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;

        let mut reports = Vec::new();
        for row in rows {
            let (
                provider_id,
                fingerprint,
                plan,
                status,
                source,
                collected_at,
                stale_at,
                error_code,
            ) = row?;
            let windows = self.windows_for(&provider_id, &collected_at)?;
            reports.push(QuotaReport {
                provider_id,
                account_fingerprint: (!fingerprint.is_empty()).then_some(fingerprint),
                plan: (!plan.is_empty()).then_some(plan),
                status: parse_status(&status),
                source,
                collected_at: parse_rfc3339(&collected_at),
                stale_at: parse_rfc3339(&stale_at),
                error_code,
                windows,
            });
        }
        Ok(reports)
    }

    /// Window snapshots for one provider collected at or after `since`,
    /// ordered oldest first.
    pub fn history(
        &self,
        provider_id: &str,
        since: DateTime<Utc>,
    ) -> Result<Vec<QuotaHistoryEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT window_key, label, scope, kind, used, quota_limit, remaining,
                    used_percent, remaining_percent, reset_at, is_unlimited, confidence, collected_at
             FROM quota_windows
             WHERE provider_id = ?1 AND collected_at >= ?2
             ORDER BY collected_at, id",
        )?;
        let rows = stmt.query_map(params![provider_id, since.to_rfc3339()], |row| {
            let reset_at: Option<String> = row.get(9)?;
            Ok(QuotaHistoryEntry {
                collected_at: parse_rfc3339(&row.get::<_, String>(12)?),
                window: QuotaWindow {
                    window_key: row.get(0)?,
                    label: row.get(1)?,
                    scope: parse_scope(&row.get::<_, String>(2)?),
                    kind: parse_kind(&row.get::<_, String>(3)?),
                    used: row.get::<_, i64>(4)? as u64,
                    limit: row.get::<_, i64>(5)? as u64,
                    remaining: row.get::<_, i64>(6)? as u64,
                    used_percent: row.get(7)?,
                    remaining_percent: row.get(8)?,
                    reset_at: reset_at.map(|s| parse_rfc3339(&s)),
                    is_unlimited: row.get::<_, i64>(10)? != 0,
                    confidence: parse_confidence(&row.get::<_, String>(11)?),
                },
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    /// Marks fresh reports whose `stale_at` is before `cutoff` as stale.
    /// Returns the number of reports updated.
    pub fn mark_stale(&self, cutoff: DateTime<Utc>) -> Result<usize, rusqlite::Error> {
        let changed = self.conn.execute(
            "UPDATE quota_reports
             SET status = 'stale'
             WHERE status = 'fresh' AND stale_at < ?1",
            [cutoff.to_rfc3339()],
        )?;
        Ok(changed)
    }

    /// Deletes window snapshots older than `before` while keeping the latest
    /// snapshot of each `(provider_id, window_key)`. Returns rows removed.
    pub fn prune(&self, before: DateTime<Utc>) -> Result<usize, rusqlite::Error> {
        let deleted = self.conn.execute(
            "DELETE FROM quota_windows
             WHERE collected_at < ?1
               AND id NOT IN (
                   SELECT MAX(id) FROM quota_windows GROUP BY provider_id, window_key
               )",
            [before.to_rfc3339()],
        )?;
        Ok(deleted)
    }

    fn windows_for(
        &self,
        provider_id: &str,
        collected_at: &str,
    ) -> Result<Vec<QuotaWindow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT window_key, label, scope, kind, used, quota_limit, remaining,
                    used_percent, remaining_percent, reset_at, is_unlimited, confidence
             FROM quota_windows
             WHERE provider_id = ?1 AND collected_at = ?2
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![provider_id, collected_at], |row| {
            let reset_at: Option<String> = row.get(9)?;
            Ok(QuotaWindow {
                window_key: row.get(0)?,
                label: row.get(1)?,
                scope: parse_scope(&row.get::<_, String>(2)?),
                kind: parse_kind(&row.get::<_, String>(3)?),
                used: row.get::<_, i64>(4)? as u64,
                limit: row.get::<_, i64>(5)? as u64,
                remaining: row.get::<_, i64>(6)? as u64,
                used_percent: row.get(7)?,
                remaining_percent: row.get(8)?,
                reset_at: reset_at.map(|s| parse_rfc3339(&s)),
                is_unlimited: row.get::<_, i64>(10)? != 0,
                confidence: parse_confidence(&row.get::<_, String>(11)?),
            })
        })?;

        let mut windows = Vec::new();
        for row in rows {
            windows.push(row?);
        }
        Ok(windows)
    }
}

fn status_to_str(status: QuotaStatus) -> &'static str {
    match status {
        QuotaStatus::Fresh => "fresh",
        QuotaStatus::Stale => "stale",
        QuotaStatus::Unavailable => "unavailable",
        QuotaStatus::AuthExpired => "auth_expired",
        QuotaStatus::RateLimited => "rate_limited",
        QuotaStatus::Error => "error",
    }
}

fn parse_status(value: &str) -> QuotaStatus {
    match value {
        "fresh" => QuotaStatus::Fresh,
        "stale" => QuotaStatus::Stale,
        "unavailable" => QuotaStatus::Unavailable,
        "auth_expired" => QuotaStatus::AuthExpired,
        "rate_limited" => QuotaStatus::RateLimited,
        _ => QuotaStatus::Error,
    }
}

fn scope_to_str(scope: QuotaWindowScope) -> &'static str {
    match scope {
        QuotaWindowScope::Rolling => "rolling",
        QuotaWindowScope::Daily => "daily",
        QuotaWindowScope::Weekly => "weekly",
        QuotaWindowScope::Monthly => "monthly",
        QuotaWindowScope::Session => "session",
        QuotaWindowScope::Other => "other",
    }
}

fn parse_scope(value: &str) -> QuotaWindowScope {
    match value {
        "rolling" => QuotaWindowScope::Rolling,
        "daily" => QuotaWindowScope::Daily,
        "weekly" => QuotaWindowScope::Weekly,
        "monthly" => QuotaWindowScope::Monthly,
        "session" => QuotaWindowScope::Session,
        _ => QuotaWindowScope::Other,
    }
}

fn kind_to_str(kind: QuotaKind) -> &'static str {
    match kind {
        QuotaKind::Requests => "requests",
        QuotaKind::Tokens => "tokens",
        QuotaKind::Credits => "credits",
        QuotaKind::Parallel => "parallel",
    }
}

fn parse_kind(value: &str) -> QuotaKind {
    match value {
        "requests" => QuotaKind::Requests,
        "tokens" => QuotaKind::Tokens,
        "credits" => QuotaKind::Credits,
        "parallel" => QuotaKind::Parallel,
        _ => QuotaKind::Requests,
    }
}

fn confidence_to_str(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

fn parse_confidence(value: &str) -> Confidence {
    match value {
        "low" => Confidence::Low,
        "high" => Confidence::High,
        _ => Confidence::Medium,
    }
}

fn parse_rfc3339(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}
