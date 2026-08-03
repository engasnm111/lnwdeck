import type { BadgeTone } from "@lnwdeck/ui";

/**
 * Freshness of the last successful collection.
 *
 * The state is derived from a real timestamp. Without one the answer is
 * "unknown", never "fresh".
 */
export interface Freshness {
  label: string;
  tone: BadgeTone;
}

/** Data older than this is stale. */
export const STALE_AFTER_MS = 15 * 60 * 1000;
/** Data older than this is treated as outdated rather than merely stale. */
export const OUTDATED_AFTER_MS = 24 * 60 * 60 * 1000;

export function freshnessOf(
  lastSuccessfulSync: string | null | undefined,
  now: number,
): Freshness {
  if (!lastSuccessfulSync) {
    return { label: "No data", tone: "neutral" };
  }
  const timestamp = Date.parse(lastSuccessfulSync);
  if (Number.isNaN(timestamp)) {
    return { label: "Unknown", tone: "neutral" };
  }
  const age = now - timestamp;
  if (age < 0) {
    return { label: "Fresh", tone: "success" };
  }
  if (age <= STALE_AFTER_MS) {
    return { label: "Fresh", tone: "success" };
  }
  if (age <= OUTDATED_AFTER_MS) {
    return { label: "Stale", tone: "warning" };
  }
  return { label: "Outdated", tone: "danger" };
}

/** Compact relative time such as "4 min ago". */
export function formatRelativeTime(value: string, now: number): string {
  const timestamp = Date.parse(value);
  if (Number.isNaN(timestamp)) {
    return "at an unknown time";
  }
  const seconds = Math.max(0, Math.round((now - timestamp) / 1000));
  if (seconds < 60) {
    return "just now";
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `${minutes} min ago`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `${hours} h ago`;
  }
  const days = Math.round(hours / 24);
  return `${days} d ago`;
}

/** Absolute local timestamp, or a dash when there is nothing to show. */
export function formatTimestamp(value: string | null | undefined): string {
  if (!value) {
    return "-";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "-";
  }
  return date.toLocaleString();
}

/** Thousands-separated integer. */
export function formatNumber(value: number): string {
  return value.toLocaleString();
}

/** Compact token counts: 1.2K, 3.4M. */
export function formatCompact(value: number): string {
  if (!Number.isFinite(value)) {
    return "-";
  }
  if (Math.abs(value) < 1000) {
    return String(value);
  }
  if (Math.abs(value) < 1_000_000) {
    return `${(value / 1000).toFixed(1)}K`;
  }
  return `${(value / 1_000_000).toFixed(1)}M`;
}
