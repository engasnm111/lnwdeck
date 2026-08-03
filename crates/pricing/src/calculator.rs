use crate::catalog::PriceResolver;

pub fn resolve_price(
    provider: &str,
    model: &str,
    resolver: &PriceResolver,
) -> Option<(String, String)> {
    let norm_prov = normalize_provider_id(provider);
    let pricing = resolver.resolve(norm_prov, model)?;
    Some((pricing.input_per_1k, pricing.output_per_1k))
}

pub fn calculate_cost(
    model: &str,
    tokens_input: u64,
    tokens_output: u64,
    resolver: &PriceResolver,
) -> Result<String, String> {
    calculate_cost_with_provider("", model, tokens_input, tokens_output, resolver)
}

pub fn calculate_cost_with_provider(
    provider: &str,
    model: &str,
    tokens_input: u64,
    tokens_output: u64,
    resolver: &PriceResolver,
) -> Result<String, String> {
    let norm_prov = if !provider.is_empty() {
        normalize_provider_id(provider)
    } else {
        infer_provider(model)
    };

    let pricing = resolver
        .resolve(norm_prov, model)
        .or_else(|| resolver.resolve("openai", model))
        .or_else(|| resolver.resolve("anthropic", model))
        .or_else(|| resolver.resolve("google", model))
        .or_else(|| resolver.resolve("kimi", model))
        .or_else(|| resolver.resolve("opencode", model))
        .ok_or_else(|| format!("unknown model pricing: {model}"))?;

    let input_rate = decimal_from_str(&pricing.input_per_1k);
    let output_rate = decimal_from_str(&pricing.output_per_1k);

    let input_cost = input_rate * (tokens_input as f64) / 1000.0;
    let output_cost = output_rate * (tokens_output as f64) / 1000.0;
    let total = input_cost + output_cost;

    Ok(format_decimal(total))
}

pub fn normalize_provider_id(provider: &str) -> &str {
    let p = provider.to_lowercase();
    if p.contains("openai") || p.contains("codex") {
        "openai"
    } else if p.contains("anthropic") || p.contains("claude") {
        "anthropic"
    } else if p.contains("google") || p.contains("gemini") {
        "google"
    } else if p.contains("kimi") || p.contains("moonshot") || p.contains("kiro") {
        "kimi"
    } else if p.contains("opencode") {
        "opencode"
    } else {
        "openai"
    }
}

fn infer_provider(model: &str) -> &str {
    let m = model.to_lowercase();
    if m.starts_with("gpt") || m.contains("codex") {
        "openai"
    } else if m.starts_with("claude") {
        "anthropic"
    } else if m.starts_with("gemini") {
        "google"
    } else if m.starts_with("moonshot") || m.starts_with("kimi") {
        "kimi"
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

    #[test]
    fn calculates_non_zero_cost_for_known_models() {
        let resolver = PriceResolver::new_with_overrides(&json!([]));
        let cost =
            calculate_cost_with_provider("anthropic", "claude-3-5-sonnet", 1000, 1000, &resolver)
                .unwrap();
        assert_eq!(cost, "0.018000");
    }
}
