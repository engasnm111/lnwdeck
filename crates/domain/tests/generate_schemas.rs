#[cfg(test)]
mod tests {
    use schemars::schema_for;
    use std::fs;

    fn schema_dir() -> String {
        format!("{}/../../schemas/domain", env!("CARGO_MANIFEST_DIR"))
    }

    fn write_schema<T: schemars::JsonSchema>(name: &str) {
        let schema = schema_for!(T);
        let json = serde_json::to_string_pretty(&schema).unwrap();
        let path = format!("{}/{}.json", schema_dir(), name);
        fs::write(&path, json).unwrap();
    }

    #[test]
    fn generate_usage_event_schema() {
        write_schema::<lnwdeck_domain::UsageEvent>("usage_event");
    }

    #[test]
    fn generate_usage_batch_schema() {
        write_schema::<lnwdeck_domain::UsageBatch>("usage_batch");
    }

    #[test]
    fn generate_quota_report_schema() {
        write_schema::<lnwdeck_domain::QuotaReport>("quota_report");
    }

    #[test]
    fn generate_quota_window_schema() {
        write_schema::<lnwdeck_domain::QuotaWindow>("quota_window");
    }

    #[test]
    fn generate_provider_descriptor_schema() {
        write_schema::<lnwdeck_domain::ProviderDescriptor>("provider_descriptor");
    }

    #[test]
    fn generate_confidence_schema() {
        write_schema::<lnwdeck_domain::Confidence>("confidence");
    }
}
