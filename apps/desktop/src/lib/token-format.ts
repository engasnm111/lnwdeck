/**
 * Token counts use a stable ASCII presentation in every UI language.
 *
 * Full values always group from the thousands position with commas. Compact
 * values use the product-wide K/M/B/T units so a user can scan dashboards
 * without locale-specific decimal or grouping surprises.
 */

const UNITS = [
  { value: 1_000_000_000_000, suffix: "T" },
  { value: 1_000_000_000, suffix: "B" },
  { value: 1_000_000, suffix: "M" },
  { value: 1_000, suffix: "K" },
] as const;

function trimDecimal(value: number): string {
  return value.toFixed(1).replace(/\.0$/, "");
}

/** Formats an exact integer token count with ASCII comma grouping. */
export function formatFullTokenCount(value: number): string {
  if (!Number.isFinite(value)) {
    return "-";
  }
  return new Intl.NumberFormat("en-US", {
    useGrouping: true,
    maximumFractionDigits: 0,
  }).format(Math.round(value));
}

/** Formats a token count as a short K/M/B/T value. */
export function formatCompactTokenCount(value: number): string {
  if (!Number.isFinite(value)) {
    return "-";
  }
  const sign = value < 0 ? "-" : "";
  const absolute = Math.abs(value);
  let unitIndex = UNITS.findIndex((candidate) => absolute >= candidate.value);
  if (unitIndex < 0) {
    return formatFullTokenCount(value);
  }

  // Rounding a value such as 999,950 to one decimal place would otherwise
  // produce the misleading `1000K`; promote it to the next unit instead.
  let scaled = absolute / UNITS[unitIndex].value;
  const rounded = Number(trimDecimal(scaled));
  if (rounded >= 1000 && unitIndex > 0) {
    unitIndex -= 1;
    scaled = absolute / UNITS[unitIndex].value;
  }
  return `${sign}${trimDecimal(scaled)}${UNITS[unitIndex].suffix}`;
}
