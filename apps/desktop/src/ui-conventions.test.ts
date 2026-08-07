import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

/**
 * Project conventions that are easy to break by accident.
 *
 * The emoji rule exists because the shipped UI must stay plain text; the
 * fabricated-status rule pins the specific strings that previously made the
 * dashboard look healthy when it had no data.
 */

const SRC = path.resolve(__dirname);
const UI_PACKAGE = path.resolve(__dirname, "../../../packages/ui/src");

function collectFiles(root: string): string[] {
  const found: string[] = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const full = path.join(root, entry.name);
    if (entry.isDirectory()) {
      found.push(...collectFiles(full));
    } else if (
      /\.(ts|tsx|css)$/.test(entry.name) &&
      // Test files reference the banned phrases as negative assertions.
      !/\.test\.(ts|tsx)$/.test(entry.name)
    ) {
      found.push(full);
    }
  }
  return found;
}

const FILES = [...collectFiles(SRC), ...collectFiles(UI_PACKAGE)];

// Emoji and pictographic ranges, plus variation selectors.
const EMOJI = /[\u{1F300}-\u{1FAFF}\u{2190}-\u{21FF}\u{2600}-\u{27BF}\u{2B00}-\u{2BFF}\u{FE0F}]/u;

describe("UI conventions", () => {
  it("finds source files to check", () => {
    expect(FILES.length).toBeGreaterThan(20);
  });

  it("contains no emoji in shipped source", () => {
    const offenders = FILES.filter((file) =>
      EMOJI.test(fs.readFileSync(file, "utf-8")),
    ).map((file) => path.relative(SRC, file));
    expect(offenders).toEqual([]);
  });

  it("does not hardcode a healthy status anywhere", () => {
    const banned = [
      "All Systems Normal",
      "Under Limit",
      "Active Ingestion",
      "All Systems Operational",
    ];
    const offenders: string[] = [];
    for (const file of FILES) {
      const content = fs.readFileSync(file, "utf-8");
      for (const phrase of banned) {
        if (content.includes(phrase)) {
          offenders.push(`${path.relative(SRC, file)}: ${phrase}`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });

  it("keeps the simulated update screen deleted", () => {
    expect(fs.existsSync(path.join(SRC, "update/UpdateView.tsx"))).toBe(false);
  });

  it("never renders a quota percentage without checking for a real limit", () => {
    const widget = fs.readFileSync(
      path.join(SRC, "windows/widget/FloatingWidget.tsx"),
      "utf-8",
    );
    expect(widget).toContain("remaining_percent === null");
  });

  it("loads no remote assets in the widget window", () => {
    const widgetRoot = path.join(SRC, "windows/widget");
    const offenders: string[] = [];
    for (const file of collectFiles(widgetRoot)) {
      const content = fs.readFileSync(file, "utf-8");
      if (
        /url\(\s*["']?https?:/.test(content) ||
        /@import\s+["']?https?:/.test(content) ||
        /\bsrc\s*=\s*["']https?:/.test(content) ||
        /\bhref\s*=\s*["']https?:/.test(content)
      ) {
        offenders.push(path.relative(SRC, file));
      }
    }
    expect(offenders).toEqual([]);
  });

  it("keeps the pet animation off under reduced motion", () => {
    const petCss = fs.readFileSync(
      path.join(SRC, "windows/widget/PetMascot.css"),
      "utf-8",
    );
    expect(petCss).toContain("@media (prefers-reduced-motion: reduce)");
    expect(petCss).toMatch(/animation:\s*none\s*!important/);
  });

  it("pins the Premium Dark Futuristic Glassmorphism palette", () => {
    // docs/DESIGN.md is the single source of truth for the brand: near-black
    // navy canvas, cyan/blue/violet accents, amber-to-red CTAs, and the glass
    // surface recipe (white 3-6%, blur, thin borders).
    const tokens = fs.readFileSync(
      path.join(SRC, "styles/tokens.css"),
      "utf-8",
    );
    const lower = tokens.toLowerCase();
    for (const color of [
      "#05070f", // near-black navy canvas
      "#22d3ee", // neon cyan primary accent
      "#2563eb", // electric blue secondary accent
      "#8b5cf6", // violet supporting accent
      "#f59e0b", // amber CTA start
      "#ef4444", // red CTA end
      "#e8edf7", // primary text
      "#8d9bb5", // secondary text
    ]) {
      expect(lower).toContain(color);
    }
    // Glass recipe: the panel fill must be translucent white, not opaque.
    expect(lower).toMatch(/--bg-panel:[^;]*rgba\(255,\s*255,\s*255,\s*0\.0[3-6]/);
    // Token names are the public contract: a redesign changes values, never names.
    expect(lower).toContain("--accent-primary");
    expect(lower).toContain("--chart-5");
  });
});
