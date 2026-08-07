/**
 * Random speech for the desktop pet.
 *
 * Quips mix real numbers from the quota dashboard (today's tokens, the
 * lowest remaining quota, plan names) with short personality lines, so every
 * tap surfaces something live. Plain text only — the bubble is decorative
 * and duplicates nothing sensitive.
 */

export interface QuipData {
  todayTokens: number;
  costUsd: number;
  currencySymbol: string;
  /** Lowest remaining percentage across published windows, if any. */
  lowestRemainingPercent: number | null;
  plan: string | null;
}

function formatCompact(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(Math.round(n));
}

function pick<T>(items: readonly T[]): T {
  return items[Math.floor(Math.random() * items.length)];
}

export function pickPetQuip(data: QuipData): string {
  const lines: string[] = [];
  if (data.todayTokens > 0) {
    lines.push(`Used ${formatCompact(data.todayTokens)} tokens today`);
    if (data.costUsd > 0) {
      lines.push(
        `${formatCompact(data.todayTokens)} tokens, ${data.currencySymbol}${data.costUsd.toFixed(2)} today`,
      );
    }
  }
  if (data.lowestRemainingPercent !== null) {
    lines.push(`${Math.round(data.lowestRemainingPercent)}% of a quota window left`);
  }
  if (data.plan) {
    lines.push(`On the ${data.plan} plan`);
  }
  lines.push(
    "Hello!",
    "Still watching your tokens...",
    "I could walk here all day",
    "Hover me anytime",
    "Click me again!",
    "Right-click for options",
  );
  return pick(lines);
}
