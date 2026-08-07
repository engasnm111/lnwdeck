import type { ProviderQuotaCard } from "../../lib/native";
import { translate } from "../../lib/i18n";

/**
 * Pet mood derivation for the floating widget.
 *
 * A pure, deterministic function over the providers currently visible after
 * `selected_providers` filtering. The caller filters; this module derives.
 * `remaining_percent` is used exactly as the backend reports it: `null` or a
 * non-finite value is skipped, never interpreted as zero or one hundred, and
 * no percentage is invented for a provider that published no limit.
 */

export type PetMood =
  | "happy" // a real percentage above 50, nothing worse
  | "worried" // a real percentage from 20 through 50
  | "critical" // a real percentage below 20
  | "stale" // a visible reading older than the freshness window
  | "error" // auth expired, rate limited, or collection failed
  | "sleeping"; // no usable percentage and no error or stale reading

/** Transient reactions layered on top of the derived mood. */
export type PetReaction = "celebrate" | null;

const ERROR_STATUSES = ["auth_expired", "rate_limited", "error"] as const;

function isUsablePercentage(value: number | null): value is number {
  return value !== null && Number.isFinite(value);
}

/**
 * Derives the pet mood with fixed precedence:
 *
 * 1. any visible provider with auth_expired, rate_limited or error status
 * 2. any real remaining percentage below 20
 * 3. any real remaining percentage from 20 through 50
 * 4. any visible provider with a stale reading
 * 5. any real remaining percentage above 50
 * 6. no usable percentage and no error or stale reading
 */
export function derivePetMood(visibleProviders: ProviderQuotaCard[]): PetMood {
  for (const provider of visibleProviders) {
    if (ERROR_STATUSES.includes(provider.status as (typeof ERROR_STATUSES)[number])) {
      return "error";
    }
  }

  // Scan every window before committing to a tier so the worst value in any
  // visible provider wins regardless of provider order.
  let sawWorried = false;
  let sawAbove50 = false;
  for (const provider of visibleProviders) {
    for (const window of provider.windows) {
      if (!isUsablePercentage(window.remaining_percent)) {
        continue;
      }
      if (window.remaining_percent < 20) {
        return "critical";
      }
      if (window.remaining_percent <= 50) {
        sawWorried = true;
      } else {
        sawAbove50 = true;
      }
    }
  }
  if (sawWorried) {
    return "worried";
  }

  for (const provider of visibleProviders) {
    if (provider.status === "stale") {
      return "stale";
    }
  }

  if (sawAbove50) {
    return "happy";
  }

  return "sleeping";
}

/** Plain-text name of a mood, shown next to the decorative mascot. */
export function petMoodLabel(
  mood: PetMood,
  t: (key: string) => string = (key) => translate("en", key),
): string {
  switch (mood) {
    case "happy":
      return t("widget.mood.happy");
    case "worried":
      return t("widget.mood.worried");
    case "critical":
      return t("widget.mood.critical");
    case "stale":
      return t("widget.mood.stale");
    case "error":
      return t("widget.mood.error");
    case "sleeping":
      return t("widget.mood.sleeping");
  }
}
