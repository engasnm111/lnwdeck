use chrono::{Duration, Utc};
use lnwdeck_domain::{
    Confidence, QuotaKind, QuotaReport, QuotaStatus, QuotaWindow, QuotaWindowScope,
};
use lnwdeck_storage::migrations::apply_all;
use lnwdeck_storage::repositories::QuotaRepository;
use lnwdeck_storage::Storage;
use tempfile::tempdir;

fn open_test_db() -> Storage {
    let dir = tempdir().expect("temp dir");
    let dir = std::mem::ManuallyDrop::new(dir);
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).expect("open");
    apply_all(&storage.conn).expect("migrate");
    storage
}

fn window(key: &str, used: u64, limit: u64) -> QuotaWindow {
    QuotaWindow::new(
        key,
        key,
        QuotaWindowScope::Weekly,
        QuotaKind::Tokens,
        used,
        limit,
        None,
        Confidence::High,
    )
}

fn report(provider_id: &str, windows: Vec<QuotaWindow>) -> QuotaReport {
    QuotaReport::new(provider_id, "fixture_api", windows, Duration::hours(1))
}

#[test]
fn upsert_and_latest_roundtrip() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);
    let report = report(
        "claude",
        vec![window("5h", 40, 100), window("7d", 300, 1000)],
    );

    repo.upsert_report(&report).expect("upsert");

    let latest = repo
        .latest_report("claude")
        .expect("latest")
        .expect("report exists");
    assert_eq!(latest.provider_id, "claude");
    assert_eq!(latest.status, QuotaStatus::Fresh);
    assert_eq!(latest.windows.len(), 2);
    assert_eq!(latest.windows[0].window_key, "5h");
    assert_eq!(latest.windows[0].remaining, 60);
    assert_eq!(latest.windows[1].window_key, "7d");
    assert!(latest.is_usable());
}

#[test]
fn upsert_older_report_does_not_clobber_newer() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);

    let first = report("claude", vec![window("5h", 40, 100)]);
    repo.upsert_report(&first).expect("first");

    let mut older = report("claude", vec![window("5h", 10, 100)]);
    older.collected_at -= Duration::hours(2);
    older.stale_at = older.collected_at + Duration::hours(1);
    let older_ts = older.collected_at;

    repo.upsert_report(&older).expect("older upsert");

    let latest = repo
        .latest_report("claude")
        .expect("latest")
        .expect("report exists");
    assert_eq!(latest.windows[0].used, 40, "newer values must win");
    assert!(
        latest.collected_at > older_ts,
        "report timestamp must not move backwards"
    );
}

#[test]
fn latest_all_returns_each_provider_once() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);
    repo.upsert_report(&report("claude", vec![window("5h", 40, 100)]))
        .expect("claude");
    repo.upsert_report(&report("codex", vec![window("7d", 10, 50)]))
        .expect("codex");
    repo.upsert_report(&report("claude", vec![window("5h", 80, 100)]))
        .expect("claude again");

    let all = repo.latest_all().expect("latest all");
    assert_eq!(all.len(), 2);
    let claude = all.iter().find(|r| r.provider_id == "claude").unwrap();
    assert_eq!(claude.windows[0].used, 80);
}

#[test]
fn history_returns_window_snapshots_in_time_range() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);
    let now = Utc::now();

    let first = report("claude", vec![window("5h", 10, 100)]);
    repo.upsert_report(&first).expect("first");

    let second = report("claude", vec![window("5h", 50, 100)]);
    repo.upsert_report(&second).expect("second");

    let history = repo
        .history("claude", now - Duration::minutes(5))
        .expect("history");
    assert_eq!(history.len(), 2);
    assert!(history[0].collected_at <= history[1].collected_at);
    assert_eq!(history[1].window.used, 50);
}

#[test]
fn mark_stale_only_marks_expired_fresh_reports() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);
    let now = Utc::now();

    let mut expired = report("claude", vec![window("5h", 40, 100)]);
    expired.collected_at = now - Duration::hours(3);
    expired.stale_at = now - Duration::hours(2);
    repo.upsert_report(&expired).expect("expired");

    let fresh = report("codex", vec![window("5h", 10, 100)]);
    repo.upsert_report(&fresh).expect("fresh");

    let marked = repo.mark_stale(now).expect("mark stale");
    assert_eq!(marked, 1);

    let claude = repo
        .latest_report("claude")
        .expect("latest")
        .expect("exists");
    assert_eq!(claude.status, QuotaStatus::Stale);
    let codex = repo
        .latest_report("codex")
        .expect("latest")
        .expect("exists");
    assert_eq!(codex.status, QuotaStatus::Fresh);
}

#[test]
fn prune_removes_old_snapshots_but_keeps_latest_per_window() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);
    let now = Utc::now();

    let mut old = report("claude", vec![window("5h", 10, 100)]);
    old.collected_at = now - Duration::hours(2);
    old.stale_at = old.collected_at + Duration::hours(1);
    repo.upsert_report(&old).expect("old");

    let new = report("claude", vec![window("5h", 50, 100)]);
    repo.upsert_report(&new).expect("new");

    let pruned = repo.prune(now - Duration::minutes(1)).expect("prune");
    assert!(pruned >= 1, "old snapshots must be pruned");

    let history = repo
        .history("claude", now - Duration::hours(2))
        .expect("history");
    assert_eq!(history.len(), 1, "latest snapshot must survive");
    assert_eq!(history[0].window.used, 50);
}

#[test]
fn error_report_roundtrip_keeps_error_code() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);
    let failed = QuotaReport::failed("claude", "cli_api", "AUTH_EXPIRED");
    repo.upsert_report(&failed).expect("upsert");

    let latest = repo
        .latest_report("claude")
        .expect("latest")
        .expect("exists");
    assert_eq!(latest.status, QuotaStatus::AuthExpired);
    assert_eq!(latest.error_code.as_deref(), Some("AUTH_EXPIRED"));
    assert!(latest.windows.is_empty());
}

#[test]
fn unknown_provider_returns_none() {
    let storage = open_test_db();
    let repo = QuotaRepository::new(&storage.conn);
    assert!(repo.latest_report("ghost").expect("latest").is_none());
}
