import { describe, expect, it } from "vitest";
import type { ProviderQuotaCard, QuotaWindowData } from "../../lib/native";
import { derivePetMood, petMoodLabel, type PetMood } from "./petState";

/**
 * The pet mood must be derived only from the providers the widget currently
 * shows, after `selected_providers` filtering has been applied by the caller.
 * Nothing here reads timestamps, invents percentages, or treats a missing
 * remaining value as zero or one hundred.
 */

function windowWith(
  remainingPercent: number | null,
  overrides: Partial<QuotaWindowData> = {},
): QuotaWindowData {
  return {
    window_key: "5h",
    label: "5-hour",
    scope: "rolling",
    kind: "tokens",
    used: 400,
    limit: remainingPercent === null ? null : 1000,
    remaining: remainingPercent === null ? null : Math.round((remainingPercent / 100) * 1000),
    used_percent: remainingPercent === null ? null : 100 - remainingPercent,
    remaining_percent: remainingPercent,
    reset_at: "2026-08-05T12:00:00Z",
    is_unlimited: false,
    confidence: "High",
    ...overrides,
  };
}

function provider(
  overrides: Partial<ProviderQuotaCard> = {},
): ProviderQuotaCard {
  return {
    provider_id: "anthropic_claude",
    display_name: "Claude",
    status: "fresh",
    plan: null,
    source: "cli_api",
    collected_at: "2026-08-05T09:00:00Z",
    stale_at: "2026-08-05T10:00:00Z",
    error_code: null,
    windows: [windowWith(72)],
    ...overrides,
  };
}

describe("derivePetMood", () => {
  it("is happy when a real percentage is above 50 and nothing is worse", () => {
    expect(derivePetMood([provider({ windows: [windowWith(72)] })])).toBe("happy");
    expect(derivePetMood([provider({ windows: [windowWith(51)] })])).toBe("happy");
    expect(derivePetMood([provider({ windows: [windowWith(100)] })])).toBe("happy");
  });

  it("is worried for a real percentage between 20 and 50 inclusive", () => {
    expect(derivePetMood([provider({ windows: [windowWith(41)] })])).toBe("worried");
    expect(derivePetMood([provider({ windows: [windowWith(20)] })])).toBe("worried");
    expect(derivePetMood([provider({ windows: [windowWith(50)] })])).toBe("worried");
  });

  it("is critical for a real percentage below 20", () => {
    expect(derivePetMood([provider({ windows: [windowWith(8)] })])).toBe("critical");
    expect(derivePetMood([provider({ windows: [windowWith(0)] })])).toBe("critical");
    expect(derivePetMood([provider({ windows: [windowWith(19.9)] })])).toBe("critical");
  });

  it("is stale when a visible reading is stale and no percentage is worse", () => {
    expect(derivePetMood([provider({ status: "stale" })])).toBe("stale");
    // A stale provider stays stale even with a healthy percentage.
    expect(derivePetMood([provider({ status: "stale", windows: [windowWith(72)] })])).toBe(
      "stale",
    );
  });

  it("is error for auth, rate limit and collection failures", () => {
    expect(
      derivePetMood([provider({ status: "auth_expired", error_code: "AUTH_EXPIRED" })]),
    ).toBe("error");
    expect(
      derivePetMood([provider({ status: "rate_limited", error_code: "RATE_LIMITED" })]),
    ).toBe("error");
    expect(
      derivePetMood([provider({ status: "error", error_code: "SOURCE_SCHEMA_MISMATCH" })]),
    ).toBe("error");
  });

  it("sleeps when no usable percentage and no error or staleness exists", () => {
    expect(derivePetMood([])).toBe("sleeping");
    expect(derivePetMood([provider({ windows: [windowWith(null)] })])).toBe("sleeping");
    expect(
      derivePetMood([
        provider({
          windows: [windowWith(null, { is_unlimited: true })],
        }),
      ]),
    ).toBe("sleeping");
  });

  it("never treats a null remaining_percent as a usable percentage", () => {
    expect(derivePetMood([provider({ windows: [windowWith(null)] })])).not.toBe("happy");
    expect(derivePetMood([provider({ windows: [windowWith(null)] })])).not.toBe("critical");
    expect(derivePetMood([provider({ windows: [windowWith(null)] })])).not.toBe("worried");
  });

  it("skips a window without a limit and still reads the others", () => {
    expect(
      derivePetMood([
        provider({ windows: [windowWith(null), windowWith(72)] }),
      ]),
    ).toBe("happy");
    expect(
      derivePetMood([
        provider({ windows: [windowWith(null), windowWith(8)] }),
      ]),
    ).toBe("critical");
  });

  it("applies precedence: error, critical, worried, stale, happy, sleeping", () => {
    const error = provider({
      status: "auth_expired",
      error_code: "AUTH_EXPIRED",
      windows: [],
    });
    const critical = provider({ provider_id: "b", windows: [windowWith(5)] });
    const worried = provider({ provider_id: "c", windows: [windowWith(35)] });
    const stale = provider({ provider_id: "d", status: "stale" });
    const healthy = provider({ provider_id: "e", windows: [windowWith(72)] });

    expect(derivePetMood([error, critical])).toBe("error");
    expect(derivePetMood([critical, worried])).toBe("critical");
    expect(derivePetMood([worried, stale])).toBe("worried");
    expect(derivePetMood([stale, healthy])).toBe("stale");
    expect(derivePetMood([healthy])).toBe("happy");
    expect(derivePetMood([provider({ provider_id: "f", windows: [windowWith(null)] })])).toBe(
      "sleeping",
    );
  });

  it("lets the worst visible quota win across several providers", () => {
    expect(
      derivePetMood([
        provider({ provider_id: "a", windows: [windowWith(72)] }),
        provider({ provider_id: "b", windows: [windowWith(41)] }),
        provider({ provider_id: "c", windows: [windowWith(8)] }),
      ]),
    ).toBe("critical");
    expect(
      derivePetMood([
        provider({ provider_id: "a", windows: [windowWith(72)] }),
        provider({ provider_id: "b", windows: [windowWith(41)] }),
      ]),
    ).toBe("worried");
    expect(
      derivePetMood([
        provider({ provider_id: "a", windows: [windowWith(60)] }),
        provider({ provider_id: "b", windows: [windowWith(55)] }),
      ]),
    ).toBe("happy");
  });

  it("ignores hidden providers because the caller filters first", () => {
    const hiddenCritical = provider({ provider_id: "hidden", windows: [windowWith(3)] });
    const visibleHealthy = provider({ provider_id: "shown", windows: [windowWith(80)] });
    expect(derivePetMood([visibleHealthy])).toBe("happy");
    expect(derivePetMood([hiddenCritical, visibleHealthy])).toBe("critical");
  });

  it("does not mutate the input provider data", () => {
    const input = [
      provider({ provider_id: "a", windows: [windowWith(72), windowWith(null)] }),
      provider({ provider_id: "b", status: "stale" }),
    ];
    const snapshot = JSON.stringify(input);
    derivePetMood(input);
    expect(JSON.stringify(input)).toBe(snapshot);
  });
});

describe("petMoodLabel", () => {
  it("names every mood with plain text", () => {
    const moods: PetMood[] = ["happy", "worried", "critical", "stale", "error", "sleeping"];
    const labels = moods.map(petMoodLabel);
    expect(labels).toEqual(["Happy", "Worried", "Critical", "Stale", "Error", "Sleeping"]);
  });
});
