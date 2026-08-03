/** Formats a reset timestamp as a compact local countdown, e.g. `2h 14m`. */
export function formatCountdown(
  resetAt: string | null,
  now: number = Date.now(),
): string | null {
  if (!resetAt) return null;
  const ms = new Date(resetAt).getTime() - now;
  if (Number.isNaN(ms)) return null;
  if (ms <= 0) return "resetting";
  const totalMinutes = Math.floor(ms / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m`;
  return `${Math.max(0, Math.floor(ms / 1000))}s`;
}

/** How long ago a report was collected, e.g. `12s ago`, `3m ago`. */
export function formatRefreshedAgo(
  collectedAt: string,
  now: number = Date.now(),
): string {
  const seconds = Math.max(
    0,
    Math.floor((now - new Date(collectedAt).getTime()) / 1000),
  );
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ago`;
}

/** Compact token/quantity formatting: 1500 -> `1.5k`, 2000000 -> `2.0M`. */
export function formatCompact(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}
