import { describe, it, expect } from "vitest";
import { formatCompact, formatCountdown, formatRefreshedAgo } from "./widgetTime";

const HOUR = 3_600_000;

describe("formatCountdown", () => {
  it("renders hours and minutes", () => {
    const now = 1_800_000_000_000;
    expect(formatCountdown(new Date(now + 134 * 60_000).toISOString(), now)).toBe(
      "2h 14m",
    );
  });

  it("renders minutes only under an hour", () => {
    const now = 1_800_000_000_000;
    expect(formatCountdown(new Date(now + 45 * 60_000).toISOString(), now)).toBe(
      "45m",
    );
  });

  it("renders seconds under a minute", () => {
    const now = 1_800_000_000_000;
    expect(formatCountdown(new Date(now + 20_000).toISOString(), now)).toBe(
      "20s",
    );
  });

  it("returns null when there is no reset time", () => {
    expect(formatCountdown(null)).toBeNull();
  });

  it("reports resetting once elapsed", () => {
    const now = 1_800_000_000_000;
    expect(formatCountdown(new Date(now - 5_000).toISOString(), now)).toBe(
      "resetting",
    );
  });
});

describe("formatRefreshedAgo", () => {
  const now = 1_800_000_000_000;

  it("renders seconds", () => {
    expect(formatRefreshedAgo(new Date(now - 12_000).toISOString(), now)).toBe(
      "12s ago",
    );
  });

  it("renders minutes", () => {
    expect(formatRefreshedAgo(new Date(now - 3 * 60_000).toISOString(), now)).toBe(
      "3m ago",
    );
  });

  it("renders hours", () => {
    expect(formatRefreshedAgo(new Date(now - 5 * HOUR).toISOString(), now)).toBe(
      "5h ago",
    );
  });
});

describe("formatCompact", () => {
  it("formats thousands", () => {
    expect(formatCompact(1500)).toBe("1.5k");
  });

  it("formats millions", () => {
    expect(formatCompact(2_000_000)).toBe("2.0M");
  });

  it("keeps small numbers as-is", () => {
    expect(formatCompact(775)).toBe("775");
  });
});
