import { useState, type ReactNode } from "react";
import {
  formatCompactTokenCount,
  formatFullTokenCount,
} from "../lib/token-format";

export interface TokenValueProps {
  value: number;
  label: string;
  exactLabel: string;
  className?: string;
  suffix?: ReactNode;
}

/**
 * Compact token value that can be expanded in place without losing the
 * context of the metric. The exact value is also exposed in the accessible
 * name while the compact value is visible.
 */
export function TokenValue({
  value,
  label,
  exactLabel,
  className = "",
  suffix,
}: TokenValueProps) {
  const [expanded, setExpanded] = useState(false);
  const compact = formatCompactTokenCount(value);
  const exact = formatFullTokenCount(value);
  const classes = ["token-value", className].filter(Boolean).join(" ");

  if (!Number.isFinite(value) || Math.abs(value) < 1_000) {
    return <span className={classes}>{exact}{suffix}</span>;
  }

  return (
    <button
      type="button"
      className={classes}
      aria-label={`${label}: ${exact}`}
      aria-expanded={expanded}
      title={exactLabel}
      onClick={() => setExpanded((current) => !current)}
      onKeyDown={(event) => {
        if (event.key === "Escape" && expanded) {
          event.preventDefault();
          setExpanded(false);
        }
      }}
    >
      {expanded ? exact : compact}{suffix}
    </button>
  );
}
