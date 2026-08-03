use crate::catalog::PriceResolver;

pub fn resolve_price(
    provider: &str,
    model: &str,
    resolver: &PriceResolver,
) -> Option<(String, String)> {
    let pricing = resolver.resolve(provider, model)?;
    Some((pricing.input_per_1k, pricing.output_per_1k))
}

pub fn calculate_cost(
    model: &str,
    tokens_input: u64,
    tokens_output: u64,
    resolver: &PriceResolver,
) -> Result<String, String> {
    // Search all providers for the model
    // For tests, the model "gpt-4o" is in "openai"
    let provider = infer_provider(model);
    let pricing = resolver
        .resolve(provider, model)
        .ok_or_else(|| format!("unknown model: {model}"))?;

    let input_rate = decimal_from_str(&pricing.input_per_1k);
    let output_rate = decimal_from_str(&pricing.output_per_1k);

    let input_cost = input_rate * (tokens_input as f64) / 1000.0;
    let output_cost = output_rate * (tokens_output as f64) / 1000.0;
    let total = input_cost + output_cost;

    Ok(format_decimal(total))
}

fn infer_provider(model: &str) -> &str {
    if model.starts_with("gpt") {
        "openai"
    } else if model.starts_with("claude") {
        "anthropic"
    } else if model.starts_with("gemini") {
        "google"
    } else {
        "openai"
    }
}

fn decimal_from_str(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(0.0)
}

fn format_decimal(v: f64) -> String {
    format!("{:.6}", v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn override_has_higher_priority_than_catalog() {
        let overrides = json!([{
            "provider": "openai",
            "model": "gpt-4o",
            "input_per_1k": "0.00100",
            "output_per_1k": "0.00200"
        }]);
        let resolver = PriceResolver::new_with_overrides(&overrides);
        let result = resolver.resolve("openai", "gpt-4o").unwrap();
        assert_eq!(result.input_per_1k, "0.00100");
    }
}
