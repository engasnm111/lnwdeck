use lnwdeck_pricing::{calculate_cost, PriceResolver, PricingStatus};
use serde_json::json;

#[test]
fn override_beats_provider_catalog() {
    let overrides = json!([{
        "provider": "openai",
        "model": "gpt-4o",
        "input_per_1k": "0.00333",
        "output_per_1k": "0.00999"
    }]);
    let resolver = PriceResolver::new_with_overrides(&overrides);

    let cost_with_override = calculate_cost("gpt-4o", 1000, 500, &resolver).unwrap();
    assert_eq!(cost_with_override.cost, "0.008325");
    assert_eq!(cost_with_override.status, PricingStatus::Priced);

    let resolver_no_override = PriceResolver::new_with_overrides(&json!([]));
    let cost_no_override = calculate_cost("gpt-4o", 1000, 500, &resolver_no_override).unwrap();
    assert_eq!(cost_no_override.cost, "0.007500");

    // Override cost must differ from catalog cost
    assert_ne!(cost_with_override.cost, cost_no_override.cost);
}

#[test]
fn provider_catalog_used_when_no_override() {
    let resolver = PriceResolver::new_with_overrides(&json!([]));
    let cost = calculate_cost("gpt-4o", 2000, 1000, &resolver).unwrap();
    // 2k input * 0.0025 + 1k output * 0.01 = 0.005 + 0.01 = 0.015
    assert_eq!(cost.cost, "0.015000");
    assert_eq!(cost.status, PricingStatus::Priced);
}

#[test]
fn missing_model_gets_a_labeled_estimate() {
    let resolver = PriceResolver::new_with_overrides(&json!([]));
    let result = calculate_cost("nonexistent-model", 100, 50, &resolver).unwrap();
    assert_eq!(result.status, PricingStatus::Estimated);
    assert!(
        result.cost.parse::<f64>().unwrap() > 0.0,
        "estimate is never zero"
    );
}

#[test]
fn cost_is_decimal_safe_small_values() {
    let resolver = PriceResolver::new_with_overrides(&json!([]));
    let cost = calculate_cost("gpt-4o", 10, 10, &resolver).unwrap();
    // 0.01 * 0.0025 + 0.01 * 0.01 = 0.000025 + 0.0001 = 0.000125
    assert_eq!(cost.cost, "0.000125");
}

#[test]
fn override_only_applies_to_specified_model() {
    let overrides = json!([{
        "provider": "openai",
        "model": "gpt-4o",
        "input_per_1k": "0.00100",
        "output_per_1k": "0.00200"
    }]);
    let resolver = PriceResolver::new_with_overrides(&overrides);

    // gpt-4o uses override
    let gpt4o = calculate_cost("gpt-4o", 1000, 1000, &resolver).unwrap();
    assert_eq!(gpt4o.cost, "0.003000");

    // gpt-4o-mini uses catalog
    let mini = calculate_cost("gpt-4o-mini", 1000, 1000, &resolver).unwrap();
    assert_eq!(mini.cost, "0.000750");
}
