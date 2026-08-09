import type { I18n } from "./i18n";

type Translate = I18n["t"];

function normalized(value: string): string {
  return value.trim().toLowerCase().replace(/[\s_-]+/g, " ");
}

function humanize(value: string): string {
  return value
    .trim()
    .replace(/[\s_-]+/g, " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}

export function providerHealthLabel(status: string, t: Translate): string {
  const value = normalized(status);
  if (value === "healthy") return t("providers.status.healthy");
  if (value === "not configured") return t("providers.status.notConfigured");
  if (value === "not supported") return t("providers.status.notSupported");
  if (value === "source not found") return t("providers.status.sourceNotFound");
  const error = /^error \((.+)\)$/i.exec(status.trim());
  if (error) return t("providers.status.error", { error: error[1] });
  return status.trim() ? humanize(status) : t("providers.status.unavailable");
}

export function providerSupportLabel(support: string, t: Translate): string {
  const value = normalized(support);
  if (value === "supported") return t("providers.support.supported");
  if (value === "local estimate") return t("providers.support.localEstimate");
  if (value === "not supported") return t("providers.support.notSupported");
  return support.trim() ? humanize(support) : t("providers.support.unknown");
}

export function providerAuthLabel(requirement: string, t: Translate): string {
  const value = normalized(requirement);
  if (value === "none") return t("providers.auth.none");
  if (value === "local files") return t("providers.auth.localFiles");
  if (value === "api key") return t("providers.auth.apiKey");
  if (value === "browser cookie") return t("providers.auth.browserCookie");
  return requirement.trim() ? humanize(requirement) : t("providers.auth.unknown");
}

export function providerCostLabel(cost: string, t: Translate): string {
  const value = normalized(cost);
  if (value === "pricing available" || value === "priced") {
    return t("providers.cost.priced");
  }
  if (value === "estimated" || value === "estimate") {
    return t("providers.cost.estimated");
  }
  if (value === "not available") return t("providers.cost.notAvailable");
  if (value === "no data") return t("providers.cost.noData");
  if (value === "missing pricing") return t("providers.cost.missingPricing");
  return cost.trim() ? humanize(cost) : t("providers.cost.noData");
}

export function providerSourceLabel(source: string, t: Translate): string {
  const value = normalized(source);
  if (
    value === "local sqlite" ||
    value === "sqlite" ||
    value === "local database" ||
    value === "local logs" ||
    value === "local files" ||
    value === "local scan" ||
    value === "local api"
  ) {
    return t("providers.source.local");
  }
  if (value === "remote api" || value === "cli api" || value === "credential") {
    return value === "credential"
      ? t("providers.source.credential")
      : t("providers.source.remote");
  }
  if (value === "local estimate") return t("providers.source.localEstimate");
  if (value === "none") return t("providers.source.none");
  return source.trim() ? humanize(source) : t("providers.source.unknown");
}

export function providerKindLabel(kind: string, t: Translate): string {
  const value = normalized(kind);
  if (value === "requests") return t("providers.kind.requests");
  if (value === "tokens") return t("providers.kind.tokens");
  if (value === "credits") return t("providers.kind.credits");
  if (value === "parallel") return t("providers.kind.parallel");
  return kind.trim() ? humanize(kind) : t("providers.kind.unknown");
}

export function providerQuotaSummaryLabel(summary: string, t: Translate): string {
  const value = summary.trim();
  const lower = value.toLowerCase();
  if (lower === "no quota data") return t("providers.quota.noData");
  if (lower === "not supported") return t("providers.quota.notSupported");
  if (lower === "no quota windows reported") return t("providers.quota.noWindows");
  if (lower === "local / unlimited") return t("providers.quota.localUnlimited");

  const left = /^(\d+(?:\.\d+)?)% left(?:\s*·\s*resets\s*(.+))?$/i.exec(value);
  if (left) {
    return left[2]
      ? t("providers.quota.reset", { percent: left[1], time: left[2] })
      : t("providers.quota.left", { percent: left[1] });
  }

  const used = /^used\s+(.+?)\s+(requests|tokens|credits|parallel)\s+\(estimate\)$/i.exec(value);
  if (used) {
    return t("providers.quota.used", {
      used: used[1],
      kind: providerKindLabel(used[2], t),
    });
  }

  const expired = /^auth expired \((.+)\)$/i.exec(value);
  if (expired) return t("providers.quota.authExpired", { error: expired[1] });
  const error = /^error \((.+)\)$/i.exec(value);
  if (error) return t("providers.quota.error", { error: error[1] });
  return value ? humanize(value) : t("providers.quota.unavailable");
}
