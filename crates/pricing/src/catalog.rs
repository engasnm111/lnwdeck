use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// Provider dot-prefixes left on some LiteLLM model ids (for example
/// `anthropic.claude-3-5-sonnet`). Short aliases such as `glm` are excluded so
/// model ids like `glm-5.2` are never mangled.
pub const MODEL_DOT_PREFIXES: &[&str] = &[
    "openai",
    "anthropic",
    "google",
    "deepseek",
    "qwen",
    "mistral",
    "xai",
    "kimi",
    "moonshot",
    "gemini",
    "claude",
    "grok",
    "codex",
    "copilot",
    "ollama",
    "openrouter",
];

#[derive(Debug, Clone, Deserialize)]
struct Catalog {
    providers: HashMap<String, ProviderEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderEntry {
    models: HashMap<String, ModelPricing>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ModelPricing {
    pub input_per_1k: String,
    pub output_per_1k: String,
}

#[derive(Debug, Clone)]
struct Override {
    provider: String,
    model: String,
    input_per_1k: String,
    output_per_1k: String,
}

/// Strips a trailing date-like suffix from a model id: `-20240620`,
/// `-20250514-t200000z` or `@20251001`. A model is a date-variant of a family
/// when the last `-`/`@` is followed by exactly eight digits.
pub fn strip_date_suffix(model: &str) -> String {
    let bytes = model.as_bytes();
    let mut end = model.len();
    let mut idx = end;
    while idx > 0 {
        idx -= 1;
        let byte = bytes[idx];
        if (byte == b'-' || byte == b'@') && idx + 9 <= end {
            let digits = &model[idx + 1..idx + 9];
            if digits.bytes().all(|b| b.is_ascii_digit()) {
                let after = &model[idx + 9..];
                if after.is_empty() || after.starts_with('-') {
                    end = idx;
                    break;
                }
            }
        }
    }
    model[..end].to_string()
}

/// Lowers a model id and strips provider prefixes and date suffixes so the
/// catalog lookup is tolerant of the identifiers providers actually emit.
/// `glm-5.2` is left untouched (the dot belongs to the version).
pub fn normalize_model(model: &str) -> String {
    let mut value = model.trim().to_lowercase();
    if let Some((prefix, rest)) = value.split_once('/') {
        let prefix_known = MODEL_DOT_PREFIXES.contains(&prefix)
            || crate::calculator::normalize_provider_id(prefix) != "unknown";
        if prefix_known {
            value = rest.to_string();
        }
    }
    if let Some(dot) = value.find('.') {
        let prefix = &value[..dot];
        if MODEL_DOT_PREFIXES.contains(&prefix) {
            value = value[dot + 1..].to_string();
        }
    }
    strip_date_suffix(&value)
}

pub struct PriceResolver {
    catalog: Catalog,
    overrides: Vec<Override>,
}

impl PriceResolver {
    pub fn new_with_overrides(overrides: &Value) -> Self {
        let catalog: Catalog =
            serde_json::from_str(include_str!("../../../assets/pricing/catalog.json"))
                .expect("catalog must be valid");

        let overrides: Vec<Override> = if overrides.is_array() {
            overrides
                .as_array()
                .unwrap()
                .iter()
                .map(|o| Override {
                    provider: o["provider"].as_str().unwrap_or_default().to_string(),
                    model: o["model"].as_str().unwrap_or_default().to_string(),
                    input_per_1k: o["input_per_1k"].as_str().unwrap_or_default().to_string(),
                    output_per_1k: o["output_per_1k"].as_str().unwrap_or_default().to_string(),
                })
                .collect()
        } else {
            vec![]
        };

        Self { catalog, overrides }
    }

    /// User override first, then an exact catalog match.
    pub fn resolve(&self, provider: &str, model: &str) -> Option<ModelPricing> {
        for ov in &self.overrides {
            if ov.provider == provider && ov.model == model {
                return Some(ModelPricing {
                    input_per_1k: ov.input_per_1k.clone(),
                    output_per_1k: ov.output_per_1k.clone(),
                });
            }
        }

        self.catalog
            .providers
            .get(provider)
            .and_then(|p| p.models.get(model))
            .cloned()
    }

    /// Tolerant catalog match: exact, date-stripped, then family prefix.
    /// Family matching only applies to versioned model ids (`claude-sonnet-4`
    /// matches `claude-sonnet-4-20250514`, never `claude-3-5-sonnet`), so a
    /// model is never priced with a different family's rate.
    pub fn resolve_catalog(&self, provider: &str, model: &str) -> Option<ModelPricing> {
        let models = &self.catalog.providers.get(provider)?.models;

        if let Some(pricing) = models.get(model) {
            return Some(pricing.clone());
        }

        let stripped = strip_date_suffix(model);
        if let Some(pricing) = models.get(&stripped) {
            return Some(pricing.clone());
        }

        if !model.contains('-') {
            return None;
        }
        let prefix = format!("{model}-");
        let mut best: Option<(usize, &ModelPricing)> = None;
        for (key, pricing) in models {
            if let Some(rest) = key.strip_prefix(&prefix) {
                if best
                    .map(|(best_len, _)| rest.len() < best_len)
                    .unwrap_or(true)
                {
                    best = Some((rest.len(), pricing));
                }
            }
        }
        best.map(|(_, pricing)| pricing.clone())
    }
}
