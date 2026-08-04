/**
 * Time and number formatting for the floating widget.
 *
 * Every function returns an explicit "unavailable" string rather than a guess:
 * a missing reset timestamp is stated as unknown, never rendered as zero or as
 * an invented countdown.
 */

/** Shown when a provider reports no reset timestamp. */
export const RESET_UNAVAILABLE = "Reset time unavailable";
/** Shown when a provider reports no remaining value. */
export const REMAINING_UNAVAILABLE = "Unavailable";

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/**
 * Formats a reset timestamp as a label.
 *
 * - no or unparsable timestamp: `Reset time unavailable`
 * - already elapsed: `Resets now`
 * - under an hour: `Resets in 14m`
 * - under a day: `Resets in 2h 14m`
 * - the next calendar day: `Resets tomorrow`
 * - further out: `Resets in 4d 8h`
 */
export function formatResetLabel(
  resetAt: string | null | undefined,
  now: number = Date.now(),
): string {
  if (!resetAt) {
    return RESET_UNAVAILABLE;
  }
  const target = new Date(resetAt).getTime();
  if (Number.isNaN(target)) {
    return RESET_UNAVAILABLE;
  }
  const remaining = target - now;
  if (remaining <= 0) {
    return "Resets now";
  }
  if (remaining < HOUR_MS) {
    const minutes = Math.max(1, Math.round(remaining / MINUTE_MS));
    return `Resets in ${minutes}m`;
  }
  if (remaining < DAY_MS) {
    const hours = Math.floor(remaining / HOUR_MS);
    const minutes = Math.floor((remaining % HOUR_MS) / MINUTE_MS);
    return `Resets in ${hours}h ${minutes}m`;
  }
  if (calendarDaysBetween(now, target) === 1) {
    return "Resets tomorrow";
  }
  const days = Math.floor(remaining / DAY_MS);
  const hours = Math.floor((remaining % DAY_MS) / HOUR_MS);
  return `Resets in ${days}d ${hours}h`;
}

/** Whole calendar days between two instants in the local time zone. */
function calendarDaysBetween(from: number, to: number): number {
  const start = new Date(from);
  const end = new Date(to);
  const startDay = new Date(
    start.getFullYear(),
    start.getMonth(),
    start.getDate(),
  ).getTime();
  const endDay = new Date(
    end.getFullYear(),
    end.getMonth(),
    end.getDate(),
  ).getTime();
  return Math.round((endDay - startDay) / DAY_MS);
}

/**
 * Short reset label for a dense row: `3h 01m`, `Wed 11:00`, or a dash when the
 * provider published no reset time.
 */
export function formatResetShort(
  resetAt: string | null | undefined,
  now: number = Date.now(),
): string {
  if (!resetAt) {
    return "--";
  }
  const target = new Date(resetAt).getTime();
  if (Number.isNaN(target)) {
    return "--";
  }
  const remaining = target - now;
  if (remaining <= 0) {
    return "now";
  }
  if (remaining < HOUR_MS) {
    return `${Math.max(1, Math.round(remaining / MINUTE_MS))}m`;
  }
  if (remaining < DAY_MS) {
    const hours = Math.floor(remaining / HOUR_MS);
    const minutes = Math.floor((remaining % HOUR_MS) / MINUTE_MS);
    return `${hours}h ${String(minutes).padStart(2, "0")}m`;
  }
  // Beyond a day a wall-clock time is easier to act on than a countdown.
  const date = new Date(target);
  const weekday = date.toLocaleDateString(undefined, { weekday: "short" });
  const time = date.toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
  return `${weekday} ${time}`;
}

/** Human description of what a window measures, shown under its name. */
export function windowSubtitle(
  scope: string,
  kind: string,
  label: string,
): string {
  const unit =
    kind === "credits"
      ? "credit"
      : kind === "tokens"
        ? "token"
        : kind === "parallel"
          ? "parallel request"
          : "request";
  switch (scope) {
    case "rolling":
      return `Sliding ${label.toLowerCase()} ${unit} window`;
    case "session":
      return `Session ${unit} window`;
    case "daily":
      return `Daily ${unit} limit`;
    case "weekly":
      return `Weekly ${unit} limit`;
    case "monthly":
      return `Monthly ${unit} limit`;
    default:
      return `${unit.charAt(0).toUpperCase()}${unit.slice(1)} limit`;
  }
}

/** Compact countdown without the `Resets` prefix, for dense rows. */
export function formatCountdown(
  resetAt: string | null,
  now: number = Date.now(),
): string | null {
  const label = formatResetLabel(resetAt, now);
  if (label === RESET_UNAVAILABLE) {
    return null;
  }
  return label.replace(/^Resets (in )?/, "");
}

/** How long ago a report was collected: `12s ago`, `3m ago`, `2h ago`. */
export function formatRefreshedAgo(
  collectedAt: string | null | undefined,
  now: number = Date.now(),
): string {
  if (!collectedAt) {
    return "never";
  }
  const collected = new Date(collectedAt).getTime();
  if (Number.isNaN(collected)) {
    return "unknown";
  }
  const seconds = Math.max(0, Math.floor((now - collected) / 1000));
  if (seconds < 60) {
    return `${seconds}s ago`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m ago`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}h ago`;
  }
  return `${Math.floor(hours / 24)}d ago`;
}

/** Compact quantities: 1500 -> `1.5k`, 2000000 -> `2.0M`. */
export function formatCompact(value: number): string {
  if (!Number.isFinite(value)) {
    return REMAINING_UNAVAILABLE;
  }
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1)}M`;
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)}k`;
  }
  return `${value}`;
}

/** Severity of a remaining-quota percentage. */
export type QuotaLevel = "normal" | "warning" | "critical";

/**
 * Maps a remaining percentage to a level:
 * above 50 is normal, 20 to 50 inclusive is a warning, below 20 is critical.
 */
export function quotaLevel(remainingPercent: number): QuotaLevel {
  if (remainingPercent < 20) {
    return "critical";
  }
  if (remainingPercent <= 50) {
    return "warning";
  }
  return "normal";
}

/** Renders the percentage label, or `Unavailable` when there is no limit. */
export function formatRemaining(remainingPercent: number | null): string {
  if (remainingPercent === null || !Number.isFinite(remainingPercent)) {
    return REMAINING_UNAVAILABLE;
  }
  const clamped = Math.max(0, Math.min(100, remainingPercent));
  return `${Math.round(clamped)}% remaining`;
}
