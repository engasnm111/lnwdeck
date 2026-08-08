use crate::catalog::{normalize_model, ModelPricing, PriceResolver};
use serde::Serialize;

/// Why a cost was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingStatus {
    /// Matched a catalog entry or a user override.
    Priced,
    /// No exact entry matched; a generic mid-market estimate was used so a
    /// price is always shown. The estimate is labeled, never silent.
    Estimated,
}

/// One calculated cost with its provenance.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CostEstimate {
    pub cost: String,
    pub status: PricingStatus,
}

/// Generic mid-market estimate per 1k tokens, used only when no catalog entry
/// or override matches. Every workload still gets a labeled cost.
pub const FALLBACK_INPUT_PER_1K: &str = "0.002500";
pub const FALLBACK_OUTPUT_PER_1K: &str = "0.010000";

/// Resolves the per-1k rates for a provider/model pair, applying overrides,
/// tolerant catalog matching, a provider-family alias for opencode GLM
/// models, and finally a generic estimate so every workload has a labeled
/// cost.
fn resolve_pricing(provider: &str, model: &str, resolver: &PriceResolver) -> ModelPricing {
    if let Some(pricing) = resolver.resolve(provider, model) {
        return pricing;
    }

    let normalized = normalize_model(model);
    if let Some(pricing) = resolver.resolve_catalog(provider, &normalized) {
        return pricing;
    }

    // opencode-go is an aggregator: it routes GLM, Claude, GPT and other
    // models under one provider id. Resolve the routed model against its own
    // family's catalog so an opencode event for a GLM model is priced with
    // the GLM rate, never with an unrelated rate.
    if provider == "opencode" {
        let family = infer_provider(&normalized);
        if let Some(pricing) = resolver.resolve_catalog(family, &normalized) {
            return pricing;
        }
    }

    ModelPricing {
        input_per_1k: FALLBACK_INPUT_PER_1K.to_string(),
        output_per_1k: FALLBACK_OUTPUT_PER_1K.to_string(),
    }
}

pub fn calculate_cost(
    model: &str,
    tokens_input: u64,
    tokens_output: u64,
    resolver: &PriceResolver,
) -> Result<CostEstimate, String> {
    calculate_cost_with_provider("", model, tokens_input, tokens_output, resolver)
}

pub fn calculate_cost_with_provider(
    provider: &str,
    model: &str,
    tokens_input: u64,
    tokens_output: u64,
    resolver: &PriceResolver,
) -> Result<CostEstimate, String> {
    let norm_prov = if !provider.is_empty() {
        normalize_provider_id(provider)
    } else {
        infer_provider(model)
    };

    let pricing = resolve_pricing(norm_prov, model, resolver);
    let status = if provider_price_known(norm_prov, model, resolver) {
        PricingStatus::Priced
    } else {
        PricingStatus::Estimated
    };

    let input_rate = decimal_from_str(&pricing.input_per_1k);
    let output_rate = decimal_from_str(&pricing.output_per_1k);

    let input_cost = input_rate * (tokens_input as f64) / 1000.0;
    let output_cost = output_rate * (tokens_output as f64) / 1000.0;
    let total = input_cost + output_cost;

    Ok(CostEstimate {
        cost: format_decimal(total),
        status,
    })
}

/// Whether the pair resolved to a real catalog/override rate (not the generic
/// fallback). Mirrors `resolve_pricing` for status labeling.
fn provider_price_known(provider: &str, model: &str, resolver: &PriceResolver) -> bool {
    if resolver.resolve(provider, model).is_some() {
        return true;
    }
    let normalized = normalize_model(model);
    if resolver.resolve_catalog(provider, &normalized).is_some() {
        return true;
    }
    if provider == "opencode" {
        let family = infer_provider(&normalized);
        if resolver.resolve_catalog(family, &normalized).is_some() {
            return true;
        }
    }
    false
}

/// Maps a provider identifier to the canonical catalog provider. Unknown
/// providers map to `"unknown"` and are never charged with an unrelated
/// provider's rates. Kiro is a separate product from Kimi Code and must not
/// be priced as Kimi.
pub fn normalize_provider_id(provider: &str) -> &str {
    let p = provider.to_lowercase();
    if p.contains("openai") || p.contains("codex") {
        "openai"
    } else if p.contains("anthropic") || p.contains("claude") {
        "anthropic"
    } else if p.contains("google") || p.contains("gemini") {
        "google"
    } else if p.contains("moonshot") || p.contains("kimi") {
        "kimi"
    } else if p.contains("zai")
        || p.contains("zcode")
        || p.contains("zhipu")
        || p.contains("bigmodel")
        || p.contains("glm")
    {
        "zai"
    } else if p.contains("opencode") {
        "opencode"
    } else if p.contains("deepseek") {
        "deepseek"
    } else if p.contains("grok") || p.contains("xai") {
        "xai"
    } else if p.contains("qwen") {
        "qwen"
    } else if p.contains("mistral") || p.contains("codestral") {
        "mistral"
    } else if p.contains("ollama") {
        "ollama"
    } else if p.contains("openrouter") {
        "openrouter"
    } else {
        "unknown"
    }
}

fn infer_provider(model: &str) -> &str {
    let m = model.to_lowercase();
    if m.starts_with("gpt") || m.contains("codex") || m.starts_with("o1") || m.starts_with("o3") {
        "openai"
    } else if m.starts_with("claude") {
        "anthropic"
    } else if m.starts_with("gemini") {
        "google"
    } else if m.starts_with("moonshot") || m.starts_with("kimi") {
        "kimi"
    } else if m.starts_with("glm") {
        "zai"
    } else if m.starts_with("deepseek") {
        "deepseek"
    } else if m.starts_with("grok") {
        "xai"
    } else if m.starts_with("qwen") {
        "qwen"
    } else if m.starts_with("mistral") || m.starts_with("codestral") {
        "mistral"
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

    fn resolver() -> PriceResolver {
        PriceResolver::new_with_overrides(&json!([]))
    }

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
        let cost =
            calculate_cost_with_provider("anthropic", "claude-3-5-sonnet", 1000, 1000, &resolver())
                .unwrap();
        assert_eq!(cost.cost, "0.018000");
        assert_eq!(cost.status, PricingStatus::Priced);
    }

    #[test]
    fn codex_turn_context_model_uses_catalog_pricing() {
        let cost =
            calculate_cost_with_provider("openai_codex", "gpt-5.3-codex", 1000, 1000, &resolver())
                .unwrap();
        assert_eq!(cost.cost, "0.015750");
        assert_eq!(cost.status, PricingStatus::Priced);
    }

    #[test]
    fn provider_alias_and_variant_models_resolve_priced() {
        let alias = calculate_cost_with_provider(
            "anthropic_claude",
            "claude-sonnet-4",
            1000,
            1000,
            &resolver(),
        )
        .unwrap();
        assert_eq!(alias.status, PricingStatus::Priced, "alias + family match");

        let date_variant = calculate_cost_with_provider(
            "anthropic_claude",
            "claude-sonnet-4-20250514",
            1000,
            1000,
            &resolver(),
        )
        .unwrap();
        assert_eq!(
            date_variant.status,
            PricingStatus::Priced,
            "date-stripped variant resolves"
        );

        let prefixed = calculate_cost_with_provider(
            "opencode",
            "anthropic/claude-sonnet-4",
            1000,
            1000,
            &resolver(),
        )
        .unwrap();
        assert_eq!(
            prefixed.status,
            PricingStatus::Priced,
            "vendor-prefixed model id resolves"
        );
    }

    #[test]
    fn opencode_models_route_to_their_own_family_rates() {
        let glm =
            calculate_cost_with_provider("opencode", "glm-5", 1000, 1000, &resolver()).unwrap();
        assert_eq!(glm.status, PricingStatus::Priced);
        let zai = calculate_cost_with_provider("zai", "glm-5", 1000, 1000, &resolver()).unwrap();
        assert_eq!(
            glm.cost, zai.cost,
            "opencode-go GLM routing must use the GLM rate"
        );

        let claude =
            calculate_cost_with_provider("opencode", "claude-sonnet-4", 1000, 1000, &resolver())
                .unwrap();
        assert_eq!(
            claude.status,
            PricingStatus::Priced,
            "opencode events for a Claude model use the Claude rate"
        );
        let anthropic =
            calculate_cost_with_provider("anthropic", "claude-sonnet-4", 1000, 1000, &resolver())
                .unwrap();
        assert_eq!(claude.cost, anthropic.cost);
    }

    #[test]
    fn unknown_models_still_get_a_labeled_estimate() {
        let cost = calculate_cost_with_provider(
            "opencode",
            "totally-unknown-model",
            1000,
            1000,
            &resolver(),
        )
        .unwrap();
        assert_eq!(cost.status, PricingStatus::Estimated);
        assert!(
            cost.cost.parse::<f64>().unwrap() > 0.0,
            "estimate is never zero"
        );
    }

    #[test]
    fn kiro_is_not_normalized_to_kimi() {
        assert_eq!(normalize_provider_id("kiro_ai"), "unknown");
        assert_eq!(normalize_provider_id("kimi_code"), "kimi");
        assert_eq!(normalize_provider_id("moonshot-v1"), "kimi");
    }

    #[test]
    fn zcode_and_glm_map_to_the_zai_catalog() {
        assert_eq!(normalize_provider_id("zcode_ai"), "zai");
        assert_eq!(normalize_provider_id("zai_glm"), "zai");
        assert_eq!(infer_provider("glm-5.2"), "zai");
    }

    #[test]
    fn unknown_provider_never_defaults_to_openai() {
        assert_eq!(normalize_provider_id("mystery_tool"), "unknown");
        assert_eq!(normalize_provider_id("anthropic_claude"), "anthropic");
    }

    #[test]
    fn kiro_is_not_priced_with_kimi_rates() {
        let kiro =
            calculate_cost_with_provider("kiro_ai", "kimi-k2-instruct", 1000, 1000, &resolver())
                .unwrap();
        let kimi =
            calculate_cost_with_provider("kimi", "kimi-k2-instruct", 1000, 1000, &resolver())
                .unwrap();
        assert_ne!(
            kiro.cost, kimi.cost,
            "Kiro must not be charged with Kimi rates"
        );
        assert_eq!(
            kiro.status,
            PricingStatus::Estimated,
            "Kiro gets a labeled estimate, never Kimi's rate"
        );
    }

    #[test]
    fn unknown_provider_is_not_charged_with_openai_rates() {
        let mystery =
            calculate_cost_with_provider("mystery_tool", "gpt-4o-mini", 1000, 1000, &resolver())
                .unwrap();
        let openai =
            calculate_cost_with_provider("openai", "gpt-4o-mini", 1000, 1000, &resolver()).unwrap();
        assert_eq!(mystery.cost, "0.012500", "generic estimate rate applies");
        assert_ne!(
            mystery.cost, openai.cost,
            "unknown provider must not fall back to OpenAI pricing"
        );
        assert_eq!(mystery.status, PricingStatus::Estimated);
        assert_eq!(openai.status, PricingStatus::Priced);
    }

    #[test]
    fn glm_versioned_dot_models_are_not_mangled() {
        assert_eq!(normalize_model("glm-5.2"), "glm-5.2");
        assert_eq!(
            normalize_model("anthropic.claude-3-5-sonnet"),
            "claude-3-5-sonnet"
        );
        assert_eq!(
            normalize_model("anthropic/claude-sonnet-4"),
            "claude-sonnet-4"
        );
        assert_eq!(
            normalize_model("claude-sonnet-4-20250514"),
            "claude-sonnet-4"
        );
        assert_eq!(normalize_model("gpt-4o@20251001"), "gpt-4o");
    }
}
