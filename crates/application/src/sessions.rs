//! Session and project usage read model.
//!
//! Groups recorded usage events by their privacy-safe project and session
//! hashes. Display names are user-entered metadata; when a hash has no stored
//! name, a generated label (`Project 01`, `Session 01`) is shown instead.
//! Events without attribution land in the empty-hash bucket, rendered as an
//! "Unassigned" project by the UI. This module never sees raw session ids or
//! folder paths - only keyed hashes produced by the adapters.

use crate::usage_history::HistoryWindow;
use chrono::{DateTime, Utc};
use lnwdeck_storage::repositories::{MetaRow, SessionRepository};
use rusqlite::Connection;
use serde::Serialize;

/// One session's recorded usage inside a project.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionUsageRow {
    pub session_hash: String,
    /// User-entered name, or a generated label such as `Session 01`.
    pub display_name: String,
    pub provider_id: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    /// Sum of provider-reported costs, formatted as a decimal string.
    pub cost: String,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
}

/// One project (folder) and the sessions recorded under it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectUsage {
    /// Keyed hash of the folder identity; `""` groups unassigned events.
    pub project_hash: String,
    /// User-entered name, or a generated label such as `Project 01`.
    pub display_name: String,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost: String,
    pub first_seen_at: Option<String>,
    pub last_seen_at: Option<String>,
    pub sessions: Vec<SessionUsageRow>,
}

/// Complete sessions read model.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionsOverview {
    pub window: HistoryWindow,
    pub generated_at: DateTime<Utc>,
    /// Lower bound applied to the query, `None` for the full history.
    pub since: Option<DateTime<Utc>>,
    pub request_count: i64,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub cost: String,
    /// Projects sorted by token usage, most first; the unassigned bucket
    /// (empty hash) is always last.
    pub projects: Vec<ProjectUsage>,
    /// Every provider seen in the window, for the filter dropdown.
    pub providers: Vec<String>,
}

/// One aggregated usage_events row, keyed by project + session hash.
#[derive(Debug, Clone)]
struct RawSessionRow {
    project_hash: String,
    session_hash: String,
    provider_id: String,
    request_count: i64,
    tokens_input: i64,
    tokens_output: i64,
    cost: f64,
    first_seen_at: Option<String>,
    last_seen_at: Option<String>,
}

fn label(rank: usize, kind: &str) -> String {
    format!("{kind} {:02}", rank)
}

pub struct QuerySessions;

impl QuerySessions {
    /// Reads session usage for a window, optionally narrowed to one provider.
    pub fn execute(
        conn: &Connection,
        window: HistoryWindow,
        provider_id: Option<&str>,
    ) -> Result<SessionsOverview, rusqlite::Error> {
        let now = Utc::now();
        let since = window.since(now);
        let since_text = since
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "0000-01-01T00:00:00+00:00".to_string());
        let provider_filter = provider_id.unwrap_or("").to_string();
        let params = rusqlite::params![since_text, provider_filter];

        let mut stmt = conn.prepare(
            "SELECT project_hash, session_hash, MAX(provider_id) AS provider_id,
                    COUNT(*),
                    COALESCE(SUM(tokens_input), 0),
                    COALESCE(SUM(tokens_output), 0),
                    COALESCE(SUM(CAST(cost AS REAL)), 0),
                    MIN(timestamp), MAX(timestamp)
             FROM usage_events
             WHERE timestamp >= ?1 AND (?2 = '' OR provider_id = ?2)
             GROUP BY project_hash, session_hash",
        )?;
        let raw_rows = stmt
            .query_map(params, |row| {
                Ok(RawSessionRow {
                    project_hash: row.get(0)?,
                    session_hash: row.get(1)?,
                    provider_id: row.get(2)?,
                    request_count: row.get(3)?,
                    tokens_input: row.get(4)?,
                    tokens_output: row.get(5)?,
                    cost: row.get(6)?,
                    first_seen_at: row.get(7)?,
                    last_seen_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let session_meta: Vec<MetaRow> = SessionRepository::new(conn).list_session_meta()?;
        let project_meta: Vec<MetaRow> = SessionRepository::new(conn).list_project_meta()?;
        let session_names: std::collections::HashMap<String, String> = session_meta
            .into_iter()
            .filter(|row| !row.display_name.is_empty())
            .map(|row| (row.hash, row.display_name))
            .collect();
        let project_names: std::collections::HashMap<String, String> = project_meta
            .into_iter()
            .filter(|row| !row.display_name.is_empty())
            .map(|row| (row.hash, row.display_name))
            .collect();

        let mut grouped: Vec<(String, Vec<RawSessionRow>)> = Vec::new();
        for row in raw_rows {
            if let Some(entry) = grouped
                .iter_mut()
                .find(|(project_hash, _)| *project_hash == row.project_hash)
            {
                entry.1.push(row);
            } else {
                grouped.push((row.project_hash.clone(), vec![row]));
            }
        }

        let mut projects: Vec<ProjectUsage> = Vec::with_capacity(grouped.len());
        let mut total_tokens_input = 0i64;
        let mut total_tokens_output = 0i64;
        let mut total_cost = 0.0f64;
        let mut total_requests = 0i64;

        for (project_hash, mut sessions) in grouped {
            // Sessions ordered by usage, most first; stable tie-break by hash.
            sessions.sort_by(|a, b| {
                let at = a.tokens_input + a.tokens_output;
                let bt = b.tokens_input + b.tokens_output;
                bt.cmp(&at)
                    .then_with(|| a.session_hash.cmp(&b.session_hash))
            });

            let mut request_count = 0i64;
            let mut tokens_input = 0i64;
            let mut tokens_output = 0i64;
            let mut cost = 0.0f64;
            let mut first_seen_at: Option<String> = None;
            let mut last_seen_at: Option<String> = None;
            let mut session_rows = Vec::with_capacity(sessions.len());
            for (rank, session) in sessions.iter().enumerate() {
                request_count += session.request_count;
                tokens_input += session.tokens_input;
                tokens_output += session.tokens_output;
                cost += session.cost;
                first_seen_at =
                    first_seen_at
                        .take()
                        .or(session.first_seen_at.clone())
                        .map(|current| {
                            let candidate = session
                                .first_seen_at
                                .clone()
                                .unwrap_or_else(|| current.clone());
                            if candidate < current {
                                candidate
                            } else {
                                current
                            }
                        });
                last_seen_at =
                    last_seen_at
                        .take()
                        .or(session.last_seen_at.clone())
                        .map(|current| {
                            let candidate = session
                                .last_seen_at
                                .clone()
                                .unwrap_or_else(|| current.clone());
                            if candidate > current {
                                candidate
                            } else {
                                current
                            }
                        });
                session_rows.push(SessionUsageRow {
                    session_hash: session.session_hash.clone(),
                    display_name: session_names
                        .get(&session.session_hash)
                        .cloned()
                        .unwrap_or_else(|| label(rank + 1, "Session")),
                    provider_id: session.provider_id.clone(),
                    request_count: session.request_count,
                    tokens_input: session.tokens_input,
                    tokens_output: session.tokens_output,
                    cost: format!("{:.6}", session.cost),
                    first_seen_at: session.first_seen_at.clone(),
                    last_seen_at: session.last_seen_at.clone(),
                });
            }

            total_requests += request_count;
            total_tokens_input += tokens_input;
            total_tokens_output += tokens_output;
            total_cost += cost;

            projects.push(ProjectUsage {
                project_hash: project_hash.clone(),
                display_name: project_names
                    .get(&project_hash)
                    .cloned()
                    .unwrap_or_else(|| {
                        if project_hash.is_empty() {
                            String::new()
                        } else {
                            label(projects.len() + 1, "Project")
                        }
                    }),
                request_count,
                tokens_input,
                tokens_output,
                cost: format!("{cost:.6}"),
                first_seen_at,
                last_seen_at,
                sessions: session_rows,
            });
        }

        // Most-used projects first; the unassigned bucket stays last.
        projects.sort_by(|a, b| {
            let at = a.tokens_input + a.tokens_output;
            let bt = b.tokens_input + b.tokens_output;
            bt.cmp(&at)
                .then_with(|| a.project_hash.cmp(&b.project_hash))
        });
        let unassigned = projects
            .iter()
            .position(|project| project.project_hash.is_empty());
        if let Some(index) = unassigned {
            let entry = projects.remove(index);
            projects.push(entry);
        }
        // Regenerate project labels now that the final order is known.
        for (rank, project) in projects.iter_mut().enumerate() {
            if !project.project_hash.is_empty()
                && !project_names.contains_key(&project.project_hash)
            {
                project.display_name = label(rank + 1, "Project");
            }
        }

        let mut provider_stmt =
            conn.prepare("SELECT DISTINCT provider_id FROM usage_events ORDER BY provider_id")?;
        let providers = provider_stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(SessionsOverview {
            window,
            generated_at: now,
            since,
            request_count: total_requests,
            tokens_input: total_tokens_input,
            tokens_output: total_tokens_output,
            cost: format!("{total_cost:.6}"),
            projects,
            providers,
        })
    }
}

/// Stores a user-entered display name for a session (metadata only).
pub struct RenameSession;

impl RenameSession {
    pub fn execute(
        conn: &Connection,
        session_hash: &str,
        display_name: &str,
    ) -> Result<(), String> {
        let name = display_name.trim();
        if name.chars().count() > 64 {
            return Err("SESSION_NAME_TOO_LONG".to_string());
        }
        SessionRepository::new(conn)
            .rename_session(session_hash, name)
            .map_err(|error| error.to_string())
    }
}

/// Stores a user-entered display name for a project (metadata only).
pub struct RenameProject;

impl RenameProject {
    pub fn execute(
        conn: &Connection,
        project_hash: &str,
        display_name: &str,
    ) -> Result<(), String> {
        let name = display_name.trim();
        if name.chars().count() > 64 {
            return Err("PROJECT_NAME_TOO_LONG".to_string());
        }
        SessionRepository::new(conn)
            .rename_project(project_hash, name)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lnwdeck_storage::{migrations::apply_all, Storage};
    use tempfile::tempdir;

    fn open_db() -> Storage {
        let dir = tempdir().expect("temp dir");
        let dir = std::mem::ManuallyDrop::new(dir);
        let storage = Storage::open(&dir.path().join("test.db")).expect("open");
        apply_all(&storage.conn).expect("migrate");
        storage
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_event(
        storage: &Storage,
        id: &str,
        provider: &str,
        model: &str,
        input: i64,
        output: i64,
        cost: &str,
        hours_ago: i64,
        session_hash: &str,
        project_hash: &str,
    ) {
        let timestamp = (Utc::now() - chrono::Duration::hours(hours_ago)).to_rfc3339();
        storage
            .conn
            .execute(
                "INSERT INTO usage_events (id, batch_id, timestamp, provider_id, model,
                     tokens_input, tokens_output, confidence, data_source, cost,
                     session_hash, project_hash)
                 VALUES (?1, 'b', ?2, ?3, ?4, ?5, ?6, 'High', 'local', ?7, ?8, ?9)",
                rusqlite::params![
                    id,
                    timestamp,
                    provider,
                    model,
                    input,
                    output,
                    cost,
                    session_hash,
                    project_hash
                ],
            )
            .expect("insert event");
    }

    fn tokens(row: &SessionUsageRow) -> i64 {
        row.tokens_input + row.tokens_output
    }

    #[test]
    fn empty_database_produces_an_empty_overview() {
        let storage = open_db();
        let overview =
            QuerySessions::execute(&storage.conn, HistoryWindow::All, None).expect("sessions");
        assert_eq!(overview.request_count, 0);
        assert!(overview.projects.is_empty());
        assert_eq!(overview.cost, "0.000000");
    }

    #[test]
    fn events_are_grouped_into_projects_and_sessions_with_generated_names() {
        let storage = open_db();
        insert_event(
            &storage, "e1", "opencode", "glm-5", 100, 50, "0.010000", 1, "s1", "p1",
        );
        insert_event(
            &storage, "e2", "opencode", "glm-5", 50, 20, "0.004000", 2, "s1", "p1",
        );
        insert_event(
            &storage, "e3", "opencode", "glm-5", 200, 80, "0.020000", 3, "s2", "p1",
        );
        insert_event(
            &storage, "e4", "claude", "claude-x", 500, 100, "0.100000", 1, "s3", "p2",
        );

        let overview =
            QuerySessions::execute(&storage.conn, HistoryWindow::All, None).expect("sessions");

        assert_eq!(overview.request_count, 4);
        assert_eq!(overview.tokens_input, 850);
        assert_eq!(overview.tokens_output, 250);
        assert_eq!(overview.projects.len(), 2);

        // p2 used the most tokens (500) and must come first.
        assert_eq!(overview.projects[0].project_hash, "p2");
        assert_eq!(overview.projects[0].display_name, "Project 01");
        assert_eq!(overview.projects[0].sessions.len(), 1);
        assert_eq!(overview.projects[0].sessions[0].display_name, "Session 01");
        assert_eq!(overview.projects[0].cost, "0.100000");

        let p1 = &overview.projects[1];
        assert_eq!(p1.project_hash, "p1");
        assert_eq!(p1.display_name, "Project 02");
        assert_eq!(p1.request_count, 3);
        assert_eq!(p1.tokens_input, 350);
        assert_eq!(p1.sessions.len(), 2, "both sessions of p1 are listed");
        assert_eq!(
            tokens(&p1.sessions[0]),
            280,
            "sessions inside a project are ordered by usage, most first"
        );
        assert_eq!(tokens(&p1.sessions[1]), 220);
        assert_eq!(p1.sessions[0].display_name, "Session 01");
        assert_eq!(p1.sessions[1].display_name, "Session 02");
    }

    #[test]
    fn events_without_attribution_land_in_the_unassigned_bucket() {
        let storage = open_db();
        insert_event(
            &storage, "e1", "opencode", "glm-5", 10, 5, "0.001000", 1, "", "",
        );
        insert_event(
            &storage, "e2", "opencode", "glm-5", 30, 5, "0.002000", 1, "s1", "",
        );

        let overview =
            QuerySessions::execute(&storage.conn, HistoryWindow::All, None).expect("sessions");

        assert_eq!(overview.projects.len(), 1);
        let bucket = &overview.projects[0];
        assert_eq!(bucket.project_hash, "");
        assert_eq!(
            bucket.display_name, "",
            "UI renders its own label for the bucket"
        );
        assert_eq!(bucket.request_count, 2);
        assert_eq!(bucket.sessions.len(), 2);
    }

    #[test]
    fn user_renames_replace_generated_names() {
        let storage = open_db();
        insert_event(
            &storage, "e1", "opencode", "glm-5", 100, 50, "0.010000", 1, "s1", "p1",
        );
        RenameProject::execute(&storage.conn, "p1", "lnwdeck").expect("rename project");
        RenameSession::execute(&storage.conn, "s1", "fix dropdown").expect("rename session");

        let overview =
            QuerySessions::execute(&storage.conn, HistoryWindow::All, None).expect("sessions");
        assert_eq!(overview.projects[0].display_name, "lnwdeck");
        assert_eq!(
            overview.projects[0].sessions[0].display_name,
            "fix dropdown"
        );
    }

    #[test]
    fn clearing_a_rename_falls_back_to_the_generated_label() {
        let storage = open_db();
        insert_event(
            &storage, "e1", "opencode", "glm-5", 100, 50, "0.010000", 1, "s1", "p1",
        );
        RenameProject::execute(&storage.conn, "p1", "lnwdeck").expect("rename project");
        RenameProject::execute(&storage.conn, "p1", "").expect("clear rename");

        let overview =
            QuerySessions::execute(&storage.conn, HistoryWindow::All, None).expect("sessions");
        assert_eq!(overview.projects[0].display_name, "Project 01");
    }

    #[test]
    fn rename_rejects_overlong_names() {
        let storage = open_db();
        let long_name = "x".repeat(65);
        assert_eq!(
            RenameSession::execute(&storage.conn, "s1", &long_name),
            Err("SESSION_NAME_TOO_LONG".to_string())
        );
        assert_eq!(
            RenameProject::execute(&storage.conn, "p1", &long_name),
            Err("PROJECT_NAME_TOO_LONG".to_string())
        );
    }

    #[test]
    fn window_bounds_and_provider_filter_apply() {
        let storage = open_db();
        insert_event(
            &storage, "recent", "opencode", "glm-5", 10, 5, "0.001000", 1, "s1", "p1",
        );
        insert_event(
            &storage,
            "old",
            "opencode",
            "glm-5",
            1000,
            500,
            "0.100000",
            24 * 10,
            "s2",
            "p1",
        );
        insert_event(
            &storage, "claude", "claude", "claude-x", 20, 5, "0.002000", 1, "s3", "p2",
        );

        let day = QuerySessions::execute(&storage.conn, HistoryWindow::Last24h, None).expect("24h");
        assert_eq!(day.request_count, 2);
        assert_eq!(day.projects.len(), 2);

        let opencode_only =
            QuerySessions::execute(&storage.conn, HistoryWindow::All, Some("opencode"))
                .expect("filtered");
        assert_eq!(opencode_only.request_count, 2);
        assert_eq!(opencode_only.projects.len(), 1);
        assert_eq!(opencode_only.projects[0].sessions.len(), 2);
    }
}
