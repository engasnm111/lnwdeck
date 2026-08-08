#!/usr/bin/env node
// Generates assets/pricing/catalog.json from the LiteLLM model price snapshot.
//
// Usage:
//   node scripts/update-pricing-catalog.mjs <litellm.json>
//
// The LiteLLM snapshot (https://github.com/BerriAI/litellm, MIT) lists per-token
// rates for thousands of models keyed as "provider/model". This script filters
// that to the providers lnwdeck tracks, normalizes provider/model identifiers
// the same way crates/pricing/src/calculator.rs does, converts rates to
// per-1k-token decimal strings, and writes the vendored offline catalog.
// The output is a generated file; edit the source snapshot or this script,
// never the catalog by hand.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUTPUT = join(REPO_ROOT, "assets/pricing/catalog.json");

const PROVIDER_ALIASES = [
  [/openai/i, "openai"],
  [/codex/i, "openai"],
  [/copilot/i, "openai"],
  [/anthropic/i, "anthropic"],
  [/claude/i, "anthropic"],
  [/google/i, "google"],
  [/gemini/i, "google"],
  [/moonshot/i, "kimi"],
  [/kimi/i, "kimi"],
  [/zai/i, "zai"],
  [/zcode/i, "zai"],
  [/zhipu/i, "zai"],
  [/bigmodel/i, "zai"],
  [/glm/i, "zai"],
  [/opencode/i, "opencode"],
  [/deepseek/i, "deepseek"],
  [/xai/i, "xai"],
  [/grok/i, "xai"],
  [/qwen/i, "qwen"],
  [/mistral/i, "mistral"],
  [/codestral/i, "mistral"],
  [/ollama/i, "ollama"],
  [/openrouter/i, "openrouter"],
];

function normalizeProvider(raw) {
  for (const [pattern, canonical] of PROVIDER_ALIASES) {
    if (pattern.test(raw)) return canonical;
  }
  return null;
}

// Provider dot-prefixes that LiteLLM leaves on some model ids
// (e.g. "anthropic.claude-3-5-sonnet"). Short aliases like "glm" are
// deliberately excluded so model ids such as "glm-5.2" are never mangled.
const DOT_PREFIXES = new Set([
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
]);

function stripDateSuffix(model) {
  return model
    .replace(/@\d{8}$/, "")
    .replace(/-\d{8}([-t].*)?$/, "")
    .replace(/-\d{4}-\d{2}-\d{2}$/, "");
}

function normalizeModel(raw) {
  const lower = raw.toLowerCase().trim();
  // LiteLLM keys are "provider/model"; keep only the model part.
  let model = lower.split("/").pop() ?? lower;
  // Strip a leading "<provider>." prefix when it names a known provider.
  const firstDot = model.indexOf(".");
  if (firstDot > 0 && firstDot < 24 && DOT_PREFIXES.has(model.slice(0, firstDot))) {
    model = model.slice(firstDot + 1);
  }
  const stripped = stripDateSuffix(model).replace(/[.:]$/, "");
  return stripped === "" ? model : stripped;
}

function round6(value) {
  const rounded = Math.round(value * 1e6) / 1e6;
  return rounded.toFixed(6);
}

export function buildCatalog(priceJson) {
  const providers = {};
  let kept = 0;
  let skipped = 0;

  for (const [key, entry] of Object.entries(priceJson)) {
    if (typeof entry !== "object" || entry === null) {
      skipped += 1;
      continue;
    }
    const inputToken = Number(entry.input_cost_per_token);
    const outputToken = Number(entry.output_cost_per_token);
    if (!Number.isFinite(inputToken) || !Number.isFinite(outputToken)) {
      skipped += 1;
      continue;
    }
    const provider = normalizeProvider(key);
    if (!provider) {
      skipped += 1;
      continue;
    }
    const model = normalizeModel(key);
    if (!model) {
      skipped += 1;
      continue;
    }
    if (!providers[provider]) providers[provider] = { models: {} };
    providers[provider].models[model] = {
      input_per_1k: round6(inputToken * 1000),
      output_per_1k: round6(outputToken * 1000),
    };
    kept += 1;
  }

  return { providers, kept, skipped };
}

function main() {
  const input = process.argv[2];
  if (!input) {
    console.error("usage: node scripts/update-pricing-catalog.mjs <litellm.json>");
    process.exit(1);
  }
  const priceJson = JSON.parse(readFileSync(input, "utf8"));
  const { providers, kept, skipped } = buildCatalog(priceJson);

  const catalog = {
    $generated_by: "scripts/update-pricing-catalog.mjs",
    $generated_from: "BerriAI/litellm model_prices_and_context_window.json (MIT)",
    providers,
  };
  writeFileSync(OUTPUT, JSON.stringify(catalog, null, 2) + "\n");
  console.log(
    `catalog.json written: ${kept} models kept, ${skipped} skipped, ` +
      `${Object.keys(providers).length} providers (${Object.keys(providers).join(", ")})`,
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
