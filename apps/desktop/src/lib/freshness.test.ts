import { describe, expect, it } from "vitest";
import {
  formatCompact,
  formatNumber,
  formatRelativeTime,
  formatTimestamp,
  freshnessOf,
  OUTDATED_AFTER_MS,
  STALE_AFTER_MS,
} from "./freshness";

const NOW = Date.parse("2026-08-04T12:00:00Z");

describe("freshnessOf", () => {
  it("reports no data when nothing has been collected", () => {
    expect(freshnessOf(null, NOW)).toEqual({ label: "No data", tone: "neutral" });
    expect(freshnessOf(undefined, NOW)).toEqual({
      label: "No data",
      tone: "neutral",
    });
  });

  it("never claims freshness for an unparsable timestamp", () => {
    expect(freshnessOf("not a date", NOW)).toEqual({
      label: "Unknown",
      tone: "neutral",
    });
  });

  it("is fresh inside the window and stale after it", () => {
    const recent = new Date(NOW - STALE_AFTER_MS + 1000).toISOString();
    expect(freshnessOf(recent, NOW).label).toBe("Fresh");

    const stale = new Date(NOW - STALE_AFTER_MS - 1000).toISOString();
    expect(freshnessOf(stale, NOW)).toEqual({ label: "Stale", tone: "warning" });
  });

  it("is outdated after a day", () => {
    const old = new Date(NOW - OUTDATED_AFTER_MS - 1000).toISOString();
    expect(freshnessOf(old, NOW)).toEqual({
      label: "Outdated",
      tone: "danger",
    });
  });
});

describe("formatters", () => {
  it("formats relative times without inventing precision", () => {
    expect(formatRelativeTime(new Date(NOW - 5000).toISOString(), NOW)).toBe(
      "just now",
    );
    expect(
      formatRelativeTime(new Date(NOW - 4 * 60_000).toISOString(), NOW),
    ).toBe("4 min ago");
    expect(
      formatRelativeTime(new Date(NOW - 3 * 3_600_000).toISOString(), NOW),
    ).toBe("3 h ago");
    expect(
      formatRelativeTime(new Date(NOW - 2 * 86_400_000).toISOString(), NOW),
    ).toBe("2 d ago");
    expect(formatRelativeTime("nonsense", NOW)).toBe("at an unknown time");
  });

  it("shows a dash rather than a fake timestamp", () => {
    expect(formatTimestamp(null)).toBe("-");
    expect(formatTimestamp(undefined)).toBe("-");
    expect(formatTimestamp("nonsense")).toBe("-");
    expect(formatTimestamp("2026-08-04T00:00:00Z")).not.toBe("-");
  });

  it("formats numbers and compact token counts", () => {
    expect(formatNumber(1234)).toBe((1234).toLocaleString());
    expect(formatCompact(999)).toBe("999");
    expect(formatCompact(1500)).toBe("1.5K");
    expect(formatCompact(2_500_000)).toBe("2.5M");
    expect(formatCompact(Number.NaN)).toBe("-");
  });
});
