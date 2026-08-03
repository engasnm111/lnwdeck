use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
struct Catalog {
    providers: HashMap<String, ProviderEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProviderEntry {
    models: HashMap<String, ModelPricing>,
}

#[derive(Debug, Clone, Deserialize)]
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
}
