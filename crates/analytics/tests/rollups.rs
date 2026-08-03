use inwdeck_analytics::rollups::{RollupBucket, RollupService};

#[test]
fn rollup_creates_hourly_buckets() {
    let buckets = RollupService::rollup_to_hourly(&sample_events());
    assert!(!buckets.is_empty(), "must produce buckets");
    assert!(
        buckets.iter().any(|b| b.provider_id == "openai"),
        "openai must appear"
    );
}

#[test]
fn rollup_sums_tokens_correctly() {
    let buckets = RollupService::rollup_to_hourly(&sample_events());
    let total_in: u64 = buckets.iter().map(|b| b.tokens_input).sum();
    let total_out: u64 = buckets.iter().map(|b| b.tokens_output).sum();
    assert_eq!(total_in, 250);
    assert_eq!(total_out, 130);
}

#[test]
fn rollup_to_daily_aggregates_hourly() {
    let hourly = RollupService::rollup_to_hourly(&sample_events());
    let daily = RollupService::rollup_to_daily(&hourly);
    assert!(!daily.is_empty());
    assert_eq!(daily.len(), 2, "two provider/model combos in the same day");
}

#[test]
fn empty_input_produces_empty_rollup() {
    let buckets = RollupService::rollup_to_hourly(&[]);
    assert!(buckets.is_empty());
}

#[test]
fn rollup_includes_bucket_id() {
    let buckets = RollupService::rollup_to_hourly(&sample_events());
    for bucket in &buckets {
        assert!(!bucket.bucket_id.is_empty(), "every bucket must have an id");
        assert!(
            !bucket.hour_key.is_empty(),
            "every bucket must have an hour_key"
        );
    }
}

fn sample_events() -> Vec<RollupBucket> {
    vec![
        RollupBucket {
            bucket_id: "".to_string(),
            hour_key: "2025-01-01T00".to_string(),
            provider_id: "openai".to_string(),
            model: "gpt-4o".to_string(),
            tokens_input: 100,
            tokens_output: 50,
            event_count: 1,
        },
        RollupBucket {
            bucket_id: "".to_string(),
            hour_key: "2025-01-01T00".to_string(),
            provider_id: "openai".to_string(),
            model: "gpt-4o".to_string(),
            tokens_input: 50,
            tokens_output: 30,
            event_count: 1,
        },
        RollupBucket {
            bucket_id: "".to_string(),
            hour_key: "2025-01-01T01".to_string(),
            provider_id: "anthropic".to_string(),
            model: "claude-3".to_string(),
            tokens_input: 100,
            tokens_output: 50,
            event_count: 1,
        },
    ]
}
