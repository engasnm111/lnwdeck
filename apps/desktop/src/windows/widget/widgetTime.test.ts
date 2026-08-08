import { describe, expect, it } from "vitest";
import {
  formatCompact,
  formatCountdown,
  formatRefreshedAgo,
  formatRemaining,
  formatResetLabel,
  formatResetShort,
  quotaLevel,
  REMAINING_UNAVAILABLE,
  RESET_UNAVAILABLE,
} from "./widgetTime";

/** A fixed local reference point: 4 August 2026, 10:00 local time. */
const NOW = new Date(2026, 7, 4, 10, 0, 0).getTime();

function inMs(ms: number): string {
  return new Date(NOW + ms).toISOString();
}

describe("formatResetLabel", () => {
  it("states that the reset time is unknown rather than guessing", () => {
    expect(formatResetLabel(null, NOW)).toBe(RESET_UNAVAILABLE);
    expect(formatResetLabel(undefined, NOW)).toBe(RESET_UNAVAILABLE);
    expect(formatResetLabel("not a timestamp", NOW)).toBe(RESET_UNAVAILABLE);
  });

  it("reports an elapsed reset as happening now", () => {
    expect(formatResetLabel(inMs(-1), NOW)).toBe("Resets now");
    expect(formatResetLabel(inMs(0), NOW)).toBe("Resets now");
  });

  it("uses minutes under an hour", () => {
    expect(formatResetLabel(inMs(14 * 60_000), NOW)).toBe("Resets in 14m");
    expect(formatResetLabel(inMs(30_000), NOW)).toBe("Resets in 1m");
  });

  it("uses hours and minutes under a day", () => {
    expect(formatResetLabel(inMs(2 * 3_600_000 + 14 * 60_000), NOW)).toBe(
      "Resets in 2h 14m",
    );
    expect(formatResetLabel(inMs(23 * 3_600_000), NOW)).toBe("Resets in 23h 0m");
  });

  it("says tomorrow when the reset falls on the next calendar day", () => {
    // 26 hours from 10:00 lands at 12:00 the next day.
    expect(formatResetLabel(inMs(26 * 3_600_000), NOW)).toBe("Resets tomorrow");
  });

  it("uses days and hours further out", () => {
    expect(formatResetLabel(inMs(4 * 86_400_000 + 8 * 3_600_000), NOW)).toBe(
      "Resets in 4d 8h",
    );
  });
});

describe("formatCountdown", () => {
  it("drops the prefix for dense rows and returns null when unknown", () => {
    expect(formatCountdown(inMs(2 * 3_600_000 + 14 * 60_000), NOW)).toBe(
      "2h 14m",
    );
    expect(formatCountdown(inMs(26 * 3_600_000), NOW)).toBe("tomorrow");
    expect(formatCountdown(null, NOW)).toBeNull();
  });
});

describe("formatRemaining", () => {
  it("renders a real percentage", () => {
    expect(formatRemaining(72)).toBe("72% remaining");
    expect(formatRemaining(41.4)).toBe("41% remaining");
    expect(formatRemaining(0)).toBe("0% remaining");
  });

  it("says unavailable when the provider publishes no limit", () => {
    expect(formatRemaining(null)).toBe(REMAINING_UNAVAILABLE);
    expect(formatRemaining(Number.NaN)).toBe(REMAINING_UNAVAILABLE);
  });

  it("clamps out-of-range input instead of showing it", () => {
    expect(formatRemaining(140)).toBe("100% remaining");
    expect(formatRemaining(-5)).toBe("0% remaining");
  });
});

describe("quotaLevel", () => {
  it("treats more than half remaining as normal", () => {
    expect(quotaLevel(100)).toBe("normal");
    expect(quotaLevel(72)).toBe("normal");
    expect(quotaLevel(50.1)).toBe("normal");
  });

  it("warns between 20 and 50 percent inclusive", () => {
    expect(quotaLevel(50)).toBe("warning");
    expect(quotaLevel(41)).toBe("warning");
    expect(quotaLevel(20)).toBe("warning");
  });

  it("is critical below 20 percent", () => {
    expect(quotaLevel(19.9)).toBe("critical");
    expect(quotaLevel(4)).toBe("critical");
    expect(quotaLevel(0)).toBe("critical");
  });
});

describe("formatRefreshedAgo", () => {
  it("formats the age of a collection", () => {
    expect(formatRefreshedAgo(inMs(-5_000), NOW)).toBe("5s ago");
    expect(formatRefreshedAgo(inMs(-3 * 60_000), NOW)).toBe("3m ago");
    expect(formatRefreshedAgo(inMs(-2 * 3_600_000), NOW)).toBe("2h ago");
    expect(formatRefreshedAgo(inMs(-3 * 86_400_000), NOW)).toBe("3d ago");
  });

  it("does not invent a time when there is none", () => {
    expect(formatRefreshedAgo(null, NOW)).toBe("never");
    expect(formatRefreshedAgo("nonsense", NOW)).toBe("unknown");
  });
});

describe("formatCompact", () => {
  it("shortens large numbers", () => {
    expect(formatCompact(999)).toBe("999");
    expect(formatCompact(1500)).toBe("1.5K");
    expect(formatCompact(2_000_000)).toBe("2M");
  });

  it("reports a non-finite value as unavailable", () => {
    expect(formatCompact(Number.NaN)).toBe(REMAINING_UNAVAILABLE);
  });
});

describe("formatResetShort", () => {
  it("uses the selected app locale for absolute reset dates", () => {
    const reset = new Date(NOW + 2 * 86_400_000 + 2 * 3_600_000).getTime();
    const date = new Date(reset);
    const expected = `${date.toLocaleDateString("th", { weekday: "short" })} ${date.toLocaleTimeString("th", { hour: "2-digit", minute: "2-digit" })}`;
    expect(formatResetShort(new Date(reset).toISOString(), NOW, undefined, "th")).toBe(expected);
  });
});
