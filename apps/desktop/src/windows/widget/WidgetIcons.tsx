/**
 * Inline icons for the widget rows.
 *
 * Drawn as SVG rather than shipped as a font or an emoji, so the widget stays
 * plain text plus vector art and inherits the current theme colour.
 */

const COMMON = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.6,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
  focusable: false,
};

/** Rolling or session window. */
export function ClockIcon() {
  return (
    <svg {...COMMON}>
      <circle cx="8" cy="8" r="6.2" />
      <path d="M8 4.6V8l2.4 1.6" />
    </svg>
  );
}

/** Weekly window. */
export function BarsIcon() {
  return (
    <svg {...COMMON}>
      <path d="M3 12.5V9" />
      <path d="M8 12.5V4.5" />
      <path d="M13 12.5V7" />
    </svg>
  );
}

/** Monthly window. */
export function CalendarIcon() {
  return (
    <svg {...COMMON}>
      <rect x="2.5" y="3.5" width="11" height="10" rx="2" />
      <path d="M2.5 6.8h11M5.8 3.5V2M10.2 3.5V2" />
    </svg>
  );
}

/** Credits or a dedicated allowance. */
export function SparkIcon() {
  return (
    <svg {...COMMON}>
      <path d="M8 2.2l1.5 3.6 3.6 1.5-3.6 1.5L8 12.4 6.5 8.8 2.9 7.3l3.6-1.5z" />
    </svg>
  );
}

/** Anything else. */
export function DotIcon() {
  return (
    <svg {...COMMON}>
      <circle cx="8" cy="8" r="3.4" />
    </svg>
  );
}
