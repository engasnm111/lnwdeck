import "./ProviderLogo.css";

export interface ProviderLogoProps {
  providerId: string;
  displayName: string;
  vendor?: string;
  small?: boolean;
  decorative?: boolean;
}

type ProviderNameInput =
  | Pick<ProviderLogoProps, "providerId" | "displayName" | "vendor">
  | { provider_id: string; display_name: string; vendor?: string };

const CANONICAL_PROVIDER_NAMES: Record<string, string> = {
  codex: "OpenAI Codex",
  openai_codex: "OpenAI Codex",
  // `opencode` is the Go implementation (billed credits/quota), while
  // `opencode_cli` is the legacy free CLI; keep them distinguishable.
  opencode: "OpenCode (Go)",
  opencode_cli: "OpenCode (Free)",
  anthropic_claude: "Claude",
  claude: "Claude",
  cursor_ide: "Cursor",
  google_gemini: "Gemini",
  github_copilot: "Copilot",
  openrouter_api: "OpenRouter",
  xai_grok: "Grok",
  kimi_code: "Kimi Code",
  kilo_code: "Kilo Code",
  kilo_cli: "Kilo CLI",
  roo_code: "Roo Code",
  zai_glm: "Z.AI",
  zcode_ai: "ZCode",
  pi_agent: "pi",
  codebuddy: "CodeBuddy",
  workbuddy: "WorkBuddy",
  hermes: "Hermes",
  mimo_code: "Mimo Code",
  omp: "oh-my-pi",
  ollama: "Ollama",
};

function normalizedProviderKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[\s_-]+local[\s_-]+sqlite$/, "")
    .replace(/[\s-]+/g, "_");
}

function withoutSourceSuffix(value: string): string {
  return value
    .trim()
    .replace(/\s*(?:[-–—_]|\s)\s*local[\s_-]+sqlite\s*$/i, "")
    .trim();
}

/**
 * Returns the user-facing provider name while keeping adapter ids out of the
 * UI. Provider marks are bundled locally so the dashboard remains offline
 * first and does not depend on a remote icon service.
 */
export function providerDisplayName(input: ProviderNameInput): string {
  const normalized = "provider_id" in input
    ? { providerId: input.provider_id, displayName: input.display_name, vendor: input.vendor }
    : input;
  const id = normalized.providerId.trim().toLowerCase();
  const key = normalizedProviderKey(id);
  const canonical = CANONICAL_PROVIDER_NAMES[key];
  if (canonical) return canonical;

  const name = withoutSourceSuffix(normalized.displayName);
  const normalizedName = name.toLowerCase().replace(/[_-]+/g, " ").trim();
  if (name && normalizedName !== id.replace(/[_-]+/g, " ").trim() && normalizedName !== key.replace(/_/g, " ")) {
    return name;
  }

  const humanized = key
    .split(/[_-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
  return humanized || withoutSourceSuffix(normalized.vendor ?? "") || "Other provider";
}

/** Returns a safe, user-facing model label without storage implementation details. */
export function modelDisplayName(model: string, unknownLabel = "Unknown model"): string {
  const value = withoutSourceSuffix(model);
  if (!value || /^unknown(?:[\s_-]+model)?$/i.test(value)) return unknownLabel;
  return value;
}

function logoKey(providerId: string): string {
  const key = providerId.trim().toLowerCase();
  if (key === "all") return "all";
  if (key === "openai_codex" || key === "codex" || key.includes("openai")) {
    return "openai";
  }
  if (key.includes("claude") || key.includes("anthropic")) return "anthropic";
  if (key.includes("gemini") || key.includes("google")) return "google-gemini";
  if (key.includes("cursor")) return "cursor";
  if (key.includes("opencode")) return "opencode";
  if (key.includes("copilot")) return "github-copilot";
  return "other";
}

export function ProviderLogo({
  providerId,
  displayName,
  vendor,
  small = false,
  decorative = false,
}: ProviderLogoProps) {
  const key = logoKey(providerId);
  const label = providerDisplayName({ providerId, displayName, vendor });

  return (
    <span
      className={`provider-logo ${small ? "provider-logo-small" : ""}`.trim()}
      data-provider={providerId.toLowerCase()}
      data-provider-logo={key}
      role={decorative ? undefined : "img"}
      aria-label={decorative ? undefined : label}
      aria-hidden={decorative ? "true" : undefined}
    >
      <img
        src={`/provider-icons/${key}.svg`}
        alt=""
        aria-hidden="true"
        draggable="false"
      />
    </span>
  );
}
