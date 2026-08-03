use inwdeck_pricing::{calculate_cost, PriceResolver};
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
    assert_eq!(cost_with_override, "0.008325");

    let resolver_no_override = PriceResolver::new_with_overrides(&json!([]));
    let cost_no_override = calculate_cost("gpt-4o", 1000, 500, &resolver_no_override).unwrap();
    assert_eq!(cost_no_override, "0.012500");

    // Override cost must differ from catalog cost
    assert_ne!(cost_with_override, cost_no_override);
}

#[test]
fn provider_catalog_used_when_no_override() {
    let resolver = PriceResolver::new_with_overrides(&json!([]));
    let cost = calculate_cost("gpt-4o", 2000, 1000, &resolver).unwrap();
    // 2k input * 0.005 + 1k output * 0.015 = 0.010 + 0.015 = 0.025
    assert_eq!(cost, "0.025000");
}

#[test]
fn missing_model_returns_unknown() {
    let resolver = PriceResolver::new_with_overrides(&json!([]));
    let result = calculate_cost("nonexistent-model", 100, 50, &resolver);
    assert!(result.is_err(), "unknown model must return error");
}

#[test]
fn cost_is_decimal_safe_small_values() {
    let resolver = PriceResolver::new_with_overrides(&json!([]));
    let cost = calculate_cost("gpt-4o", 1, 1, &resolver).unwrap();
    // 0.001 * 0.005 + 0.001 * 0.015 = 0.000020
    assert_eq!(cost, "0.000020");
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
    assert_eq!(gpt4o, "0.003000");

    // gpt-4o-mini uses catalog
    let mini = calculate_cost("gpt-4o-mini", 1000, 1000, &resolver).unwrap();
    assert_eq!(mini, "0.000750");
}
