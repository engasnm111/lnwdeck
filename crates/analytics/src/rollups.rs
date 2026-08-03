use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RollupBucket {
    pub bucket_id: String,
    pub hour_key: String,
    pub provider_id: String,
    pub model: String,
    pub tokens_input: u64,
    pub tokens_output: u64,
    pub event_count: u64,
}

pub struct RollupService;

impl RollupService {
    pub fn rollup_to_hourly(events: &[RollupBucket]) -> Vec<RollupBucket> {
        let mut grouped: HashMap<String, RollupBucket> = HashMap::new();

        for event in events {
            let key = format!("{}_{}_{}", event.hour_key, event.provider_id, event.model);
            let entry = grouped.entry(key).or_insert_with(|| RollupBucket {
                bucket_id: String::new(),
                hour_key: event.hour_key.clone(),
                provider_id: event.provider_id.clone(),
                model: event.model.clone(),
                tokens_input: 0,
                tokens_output: 0,
                event_count: 0,
            });
            entry.tokens_input += event.tokens_input;
            entry.tokens_output += event.tokens_output;
            entry.event_count += event.event_count;
        }

        let mut result: Vec<RollupBucket> = grouped.into_values().collect();
        for (i, bucket) in result.iter_mut().enumerate() {
            bucket.bucket_id = format!("hr_{}", i);
        }
        result
    }

    pub fn rollup_to_daily(hourly: &[RollupBucket]) -> Vec<RollupBucket> {
        let mut grouped: HashMap<String, RollupBucket> = HashMap::new();

        for bucket in hourly {
            let day_key = &bucket.hour_key[..10];
            let key = format!("{}_{}_{}", day_key, bucket.provider_id, bucket.model);
            let entry = grouped.entry(key).or_insert_with(|| RollupBucket {
                bucket_id: String::new(),
                hour_key: day_key.to_string(),
                provider_id: bucket.provider_id.clone(),
                model: bucket.model.clone(),
                tokens_input: 0,
                tokens_output: 0,
                event_count: 0,
            });
            entry.tokens_input += bucket.tokens_input;
            entry.tokens_output += bucket.tokens_output;
            entry.event_count += bucket.event_count;
        }

        let mut result: Vec<RollupBucket> = grouped.into_values().collect();
        for (i, bucket) in result.iter_mut().enumerate() {
            bucket.bucket_id = format!("day_{}", i);
        }
        result
    }
}
