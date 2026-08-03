use chrono::Utc;
use lnwdeck_application::{
    overview::QueryOverview,
    providers::{ScanProviders, STANDARD_PROVIDERS},
};
use lnwdeck_domain::{Confidence, UsageBatch, UsageEvent};
use lnwdeck_pricing::{calculator::calculate_cost_with_provider, catalog::PriceResolver};
use lnwdeck_storage::{migrations::apply_all, repositories::UsageRepository, Storage};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn provider_registry_includes_codex_gemini_kimi_claude() {
    let provider_ids: Vec<&str> = STANDARD_PROVIDERS.iter().map(|p| p.id).collect();
    assert!(
        provider_ids.contains(&"openai_codex"),
        "Codex must be in provider registry"
    );
    assert!(
        provider_ids.contains(&"google_gemini"),
        "Gemini must be in provider registry"
    );
    assert!(
        provider_ids.contains(&"kiro_ai"),
        "Kiro must be in provider registry"
    );
    assert!(
        provider_ids.contains(&"anthropic_claude"),
        "Claude must be in provider registry"
    );
    assert!(
        provider_ids.contains(&"opencode"),
        "OpenCode must be in provider registry"
    );
}

#[test]
fn provider_summary_query_returns_multiple_providers() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();

    let list = ScanProviders::execute(&storage.conn).unwrap();
    assert!(
        list.len() >= 5,
        "ScanProviders must return multiple providers including Codex, Gemini, Kimi, Claude"
    );
    let ids: Vec<String> = list.into_iter().map(|p| p.provider_id).collect();
    assert!(ids.contains(&"openai_codex".to_string()));
    assert!(ids.contains(&"google_gemini".to_string()));
    assert!(ids.contains(&"kiro_ai".to_string()));
    assert!(ids.contains(&"anthropic_claude".to_string()));
}

#[test]
fn cost_calculation_from_stored_usage_events() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();

    let repo = UsageRepository::new(&storage.conn);
    let batch = UsageBatch {
        batch_id: "batch_1".to_string(),
        events: vec![UsageEvent {
            id: "evt_1".to_string(),
            timestamp: Utc::now(),
            provider_id: "anthropic_claude".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            data_source: "file_log".to_string(),
            tokens_input: 1000,
            tokens_output: 1000,
            cost: "0.018000".to_string(),
            confidence: Confidence::High,
        }],
    };
    repo.ingest_batch(&batch).unwrap();

    let overview = QueryOverview::execute(&storage.conn).unwrap();
    assert_eq!(overview.total_events, 1);
    assert!(
        overview.total_cost > 0.0,
        "Total cost must be non-zero when cost exists"
    );
    assert_eq!(overview.cost_status, "estimated");
}

#[test]
fn missing_pricing_behavior_labeled_correctly() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let storage = Storage::open(&db_path).unwrap();
    apply_all(&storage.conn).unwrap();

    let repo = UsageRepository::new(&storage.conn);
    let batch = UsageBatch {
        batch_id: "batch_2".to_string(),
        events: vec![UsageEvent {
            id: "evt_2".to_string(),
            timestamp: Utc::now(),
            provider_id: "unknown_prov".to_string(),
            model: "unknown_unpriced_model_xyz".to_string(),
            data_source: "file_log".to_string(),
            tokens_input: 100,
            tokens_output: 100,
            cost: "0.00".to_string(),
            confidence: Confidence::Medium,
        }],
    };
    repo.ingest_batch(&batch).unwrap();

    let resolver = PriceResolver::new_with_overrides(&json!([]));
    let calculated = calculate_cost_with_provider(
        "unknown_prov",
        "unknown_unpriced_model_xyz",
        100,
        100,
        &resolver,
    );
    assert!(
        calculated.is_err(),
        "Unknown model must fail pricing resolution gracefully"
    );

    let overview = QueryOverview::execute(&storage.conn).unwrap();
    assert_eq!(overview.cost_status, "missing_pricing");
    assert_eq!(overview.cost_formatted, "Unavailable");
}
