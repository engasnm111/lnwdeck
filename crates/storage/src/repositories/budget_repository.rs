//! Budget persistence.
//!
//! Budgets are entirely user-configured. Nothing is seeded, so an empty table
//! means "no budget configured" and the UI says exactly that instead of
//! showing a reassuring default.

use rusqlite::{params, Connection, OptionalExtension};

/// Period a budget is measured over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl BudgetPeriod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            _ => None,
        }
    }

    /// Length of the period in days, used to compute the window start.
    pub fn days(self) -> i64 {
        match self {
            Self::Daily => 1,
            Self::Weekly => 7,
            Self::Monthly => 30,
        }
    }
}

/// What a budget applies to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "provider_id")]
pub enum BudgetScope {
    /// All providers together.
    Global,
    /// One provider, by canonical id.
    Provider(String),
}

impl BudgetScope {
    fn parts(&self) -> (&'static str, &str) {
        match self {
            Self::Global => ("global", ""),
            Self::Provider(id) => ("provider", id.as_str()),
        }
    }

    fn from_parts(scope: &str, provider_id: &str) -> Self {
        match scope {
            "provider" => Self::Provider(provider_id.to_string()),
            _ => Self::Global,
        }
    }
}

/// A stored budget. `cost_limit` is a decimal string so money is never held as
/// a float; `token_limit` is optional. At least one of the two must be set,
/// which `BudgetRepository::upsert` enforces.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BudgetRow {
    pub id: i64,
    pub scope: BudgetScope,
    pub period: BudgetPeriod,
    /// Cost cap as a decimal string, empty when unset.
    pub cost_limit: String,
    pub token_limit: Option<u64>,
    /// Percentage of the limit that raises a warning (1..=100).
    pub warn_percent: u8,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub struct BudgetRepository<'a> {
    conn: &'a Connection,
}

impl<'a> BudgetRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Inserts or replaces a budget for its (scope, provider, period) key.
    /// Returns the row id.
    ///
    /// A budget with neither a cost nor a token limit is rejected: it could
    /// never be evaluated and would render as a meaningless progress bar.
    pub fn upsert(&self, budget: &BudgetRow) -> Result<i64, BudgetError> {
        let cost_limit = budget.cost_limit.trim();
        if cost_limit.is_empty() && budget.token_limit.is_none() {
            return Err(BudgetError::NoLimit);
        }
        if !cost_limit.is_empty() {
            let parsed: f64 = cost_limit.parse().map_err(|_| BudgetError::InvalidCost)?;
            if !parsed.is_finite() || parsed <= 0.0 {
                return Err(BudgetError::InvalidCost);
            }
        }
        if budget.token_limit == Some(0) {
            return Err(BudgetError::InvalidTokenLimit);
        }
        if budget.warn_percent == 0 || budget.warn_percent > 100 {
            return Err(BudgetError::InvalidWarnPercent);
        }
        if let BudgetScope::Provider(id) = &budget.scope {
            if id.trim().is_empty() {
                return Err(BudgetError::MissingProvider);
            }
        }

        let (scope, provider_id) = budget.scope.parts();
        let now = chrono::Utc::now().to_rfc3339();
        self.conn
            .execute(
                "INSERT INTO budgets
                    (scope, provider_id, period, cost_limit, token_limit, warn_percent,
                     enabled, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                 ON CONFLICT(scope, provider_id, period) DO UPDATE SET
                     cost_limit = excluded.cost_limit,
                     token_limit = excluded.token_limit,
                     warn_percent = excluded.warn_percent,
                     enabled = excluded.enabled,
                     updated_at = excluded.updated_at",
                params![
                    scope,
                    provider_id,
                    budget.period.as_str(),
                    cost_limit,
                    budget.token_limit.map(|value| value as i64),
                    budget.warn_percent as i64,
                    budget.enabled as i64,
                    now,
                ],
            )
            .map_err(BudgetError::Storage)?;

        self.conn
            .query_row(
                "SELECT id FROM budgets WHERE scope = ?1 AND provider_id = ?2 AND period = ?3",
                params![scope, provider_id, budget.period.as_str()],
                |row| row.get(0),
            )
            .map_err(BudgetError::Storage)
    }

    /// All budgets, newest first by update time.
    pub fn list(&self) -> Result<Vec<BudgetRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scope, provider_id, period, cost_limit, token_limit, warn_percent,
                    enabled, created_at, updated_at
             FROM budgets ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([], map_budget)?;
        let mut budgets = Vec::new();
        for row in rows {
            budgets.push(row?);
        }
        Ok(budgets)
    }

    pub fn get(&self, id: i64) -> Result<Option<BudgetRow>, rusqlite::Error> {
        self.conn
            .query_row(
                "SELECT id, scope, provider_id, period, cost_limit, token_limit, warn_percent,
                        enabled, created_at, updated_at
                 FROM budgets WHERE id = ?1",
                [id],
                map_budget,
            )
            .optional()
    }

    /// Deletes a budget. Returns false when the id did not exist, so the
    /// caller can report that instead of pretending it deleted something.
    pub fn delete(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let removed = self
            .conn
            .execute("DELETE FROM budgets WHERE id = ?1", [id])?;
        Ok(removed > 0)
    }
}

fn map_budget(row: &rusqlite::Row<'_>) -> Result<BudgetRow, rusqlite::Error> {
    let scope: String = row.get(1)?;
    let provider_id: String = row.get(2)?;
    let period: String = row.get(3)?;
    Ok(BudgetRow {
        id: row.get(0)?,
        scope: BudgetScope::from_parts(&scope, &provider_id),
        period: BudgetPeriod::parse(&period).unwrap_or(BudgetPeriod::Monthly),
        cost_limit: row.get(4)?,
        token_limit: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
        warn_percent: row.get::<_, i64>(6)?.clamp(1, 100) as u8,
        enabled: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// Why a budget was rejected. Reported to the user verbatim.
#[derive(Debug)]
pub enum BudgetError {
    NoLimit,
    InvalidCost,
    InvalidTokenLimit,
    InvalidWarnPercent,
    MissingProvider,
    Storage(rusqlite::Error),
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoLimit => write!(f, "a budget needs a cost limit or a token limit"),
            Self::InvalidCost => write!(f, "the cost limit must be a positive decimal number"),
            Self::InvalidTokenLimit => write!(f, "the token limit must be greater than zero"),
            Self::InvalidWarnPercent => {
                write!(f, "the warning threshold must be between 1 and 100")
            }
            Self::MissingProvider => write!(f, "a provider budget needs a provider id"),
            Self::Storage(error) => write!(f, "storage error: {error}"),
        }
    }
}

impl std::error::Error for BudgetError {}

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

    fn budget(scope: BudgetScope, cost: &str, tokens: Option<u64>) -> BudgetRow {
        BudgetRow {
            id: 0,
            scope,
            period: BudgetPeriod::Monthly,
            cost_limit: cost.to_string(),
            token_limit: tokens,
            warn_percent: 80,
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[test]
    fn table_starts_empty() {
        let storage = open_db();
        let repo = BudgetRepository::new(&storage.conn);
        assert!(
            repo.list().expect("list").is_empty(),
            "no budget may be seeded"
        );
    }

    #[test]
    fn upsert_then_read_roundtrip() {
        let storage = open_db();
        let repo = BudgetRepository::new(&storage.conn);
        let id = repo
            .upsert(&budget(BudgetScope::Global, "25.00", None))
            .expect("insert");
        assert!(id > 0);

        let stored = repo.get(id).expect("get").expect("row exists");
        assert_eq!(stored.scope, BudgetScope::Global);
        assert_eq!(stored.period, BudgetPeriod::Monthly);
        assert_eq!(stored.cost_limit, "25.00");
        assert_eq!(stored.token_limit, None);
        assert_eq!(stored.warn_percent, 80);
        assert!(stored.enabled);
        assert!(!stored.created_at.is_empty());
    }

    #[test]
    fn upsert_replaces_the_same_scope_and_period() {
        let storage = open_db();
        let repo = BudgetRepository::new(&storage.conn);
        let first = repo
            .upsert(&budget(
                BudgetScope::Provider("opencode".into()),
                "10",
                None,
            ))
            .expect("insert");
        let second = repo
            .upsert(&budget(
                BudgetScope::Provider("opencode".into()),
                "20",
                None,
            ))
            .expect("update");
        assert_eq!(first, second, "the same key updates in place");
        assert_eq!(repo.list().expect("list").len(), 1);
        assert_eq!(repo.get(first).expect("get").unwrap().cost_limit, "20");
    }

    #[test]
    fn invalid_budgets_are_rejected_with_a_reason() {
        let storage = open_db();
        let repo = BudgetRepository::new(&storage.conn);

        let no_limit = repo.upsert(&budget(BudgetScope::Global, "", None));
        assert!(matches!(no_limit, Err(BudgetError::NoLimit)));

        let bad_cost = repo.upsert(&budget(BudgetScope::Global, "abc", None));
        assert!(matches!(bad_cost, Err(BudgetError::InvalidCost)));

        let negative = repo.upsert(&budget(BudgetScope::Global, "-5", None));
        assert!(matches!(negative, Err(BudgetError::InvalidCost)));

        let zero_tokens = repo.upsert(&budget(BudgetScope::Global, "", Some(0)));
        assert!(matches!(zero_tokens, Err(BudgetError::InvalidTokenLimit)));

        let bad_warn = repo.upsert(&BudgetRow {
            warn_percent: 0,
            ..budget(BudgetScope::Global, "10", None)
        });
        assert!(matches!(bad_warn, Err(BudgetError::InvalidWarnPercent)));

        let no_provider = repo.upsert(&budget(BudgetScope::Provider("  ".into()), "10", None));
        assert!(matches!(no_provider, Err(BudgetError::MissingProvider)));

        assert!(
            repo.list().expect("list").is_empty(),
            "no invalid budget may be stored"
        );
    }

    #[test]
    fn token_only_budgets_are_allowed() {
        let storage = open_db();
        let repo = BudgetRepository::new(&storage.conn);
        let id = repo
            .upsert(&budget(BudgetScope::Global, "", Some(1_000_000)))
            .expect("insert");
        let stored = repo.get(id).expect("get").expect("row");
        assert_eq!(stored.token_limit, Some(1_000_000));
        assert!(stored.cost_limit.is_empty());
    }

    #[test]
    fn delete_reports_whether_a_row_was_removed() {
        let storage = open_db();
        let repo = BudgetRepository::new(&storage.conn);
        let id = repo
            .upsert(&budget(BudgetScope::Global, "10", None))
            .expect("insert");
        assert!(repo.delete(id).expect("delete"));
        assert!(!repo.delete(id).expect("second delete"));
        assert!(repo.get(id).expect("get").is_none());
    }

    #[test]
    fn periods_roundtrip_through_storage() {
        let storage = open_db();
        let repo = BudgetRepository::new(&storage.conn);
        for period in [
            BudgetPeriod::Daily,
            BudgetPeriod::Weekly,
            BudgetPeriod::Monthly,
        ] {
            let id = repo
                .upsert(&BudgetRow {
                    period,
                    ..budget(BudgetScope::Global, "10", None)
                })
                .expect("insert");
            assert_eq!(repo.get(id).expect("get").unwrap().period, period);
        }
        assert_eq!(repo.list().expect("list").len(), 3);
    }
}
