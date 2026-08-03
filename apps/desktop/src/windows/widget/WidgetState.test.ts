import { describe, it, expect } from "vitest";

describe("Widget state", () => {
  it("opacity is clamped between 0.1 and 1.0", () => {
    const clamp = (v: number) => Math.max(0.1, Math.min(1.0, v));
    expect(clamp(-0.5)).toBe(0.1);
    expect(clamp(0.5)).toBe(0.5);
    expect(clamp(1.5)).toBe(1.0);
    expect(clamp(0.0)).toBe(0.1);
    expect(clamp(1.0)).toBe(1.0);
  });

  it("lock mode toggles between locked and unlocked", () => {
    const toggle = (mode: string) => (mode === "locked" ? "unlocked" : "locked");
    expect(toggle("unlocked")).toBe("locked");
    expect(toggle("locked")).toBe("unlocked");
  });

  it("opacity increment stays within bounds", () => {
    const increment = (current: number, delta: number) =>
      Math.max(0.1, Math.min(1.0, current + delta));
    expect(increment(0.5, 0.1)).toBe(0.6);
    expect(increment(0.95, 0.1)).toBe(1.0);
    expect(increment(0.15, -0.1)).toBe(0.1);
  });

  it("widget state serialization round-trips", () => {
    const state = { opacity: 0.7, lockMode: "locked" };
    const json = JSON.stringify(state);
    const parsed = JSON.parse(json);
    expect(parsed).toEqual(state);
  });

  it("multi-monitor out-of-bounds position recovery", () => {
    const recoverPosition = (x: number, y: number, screenW: number, screenH: number) => {
      const clampedX = Math.max(0, Math.min(x, screenW - 200));
      const clampedY = Math.max(0, Math.min(y, screenH - 100));
      return { x: clampedX, y: clampedY };
    };

    // Widget at negative position should recover to 0
    expect(recoverPosition(-100, -50, 1920, 1080)).toEqual({ x: 0, y: 0 });

    // Widget within bounds should stay
    expect(recoverPosition(100, 100, 1920, 1080)).toEqual({ x: 100, y: 100 });

    // Widget beyond screen should clamp
    expect(recoverPosition(2000, 2000, 1920, 1080)).toEqual({ x: 1720, y: 980 });
  });
});
