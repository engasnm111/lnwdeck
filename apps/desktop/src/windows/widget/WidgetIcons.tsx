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

/** Refresh or sync action. */
export function RefreshIcon() {
  return (
    <svg {...COMMON}>
      <path d="M13 5.5V2.8l-2.7 2.7" />
      <path d="M13 5.5A5.5 5.5 0 1 0 13.2 9" />
    </svg>
  );
}

/** Open the main dashboard window. */
export function ExternalLinkIcon() {
  return (
    <svg {...COMMON}>
      <path d="M9.5 2.5h4v4" />
      <path d="M8 8l5.5-5.5" />
      <path d="M13.5 8.5v4a1 1 0 0 1-1 1h-8a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1h4" />
    </svg>
  );
}

/** Lock state action. */
export function LockIcon({ locked }: { locked: boolean }) {
  return (
    <svg {...COMMON}>
      <rect x="3.5" y="7" width="9" height="6.5" rx="1.5" />
      {locked ? <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" /> : <path d="M5.5 7V5.5a2.5 2.5 0 0 1 4.7-1.2" />}
    </svg>
  );
}

/** Provider filter action. */
export function FilterIcon() {
  return (
    <svg {...COMMON}>
      <path d="M2.5 3.5h11L9.2 8.8v3.7l-2.4 1V8.8z" />
    </svg>
  );
}

/** Close action. */
export function CloseIcon() {
  return (
    <svg {...COMMON}>
      <path d="m4 4 8 8M12 4l-8 8" />
    </svg>
  );
}

/** Current widget layout icon. */
export function WidgetLayoutIcon({ view }: { view: "bars" | "rings" | "pet" }) {
  if (view === "rings") {
    return (
      <svg {...COMMON}>
        <circle cx="8" cy="8" r="5.5" />
        <circle cx="8" cy="8" r="2.2" />
      </svg>
    );
  }
  if (view === "pet") {
    return (
      <svg {...COMMON}>
        <circle cx="8" cy="8.3" r="3.6" />
        <path d="M5.2 5.2 4.4 3.3l2 .7M10.8 5.2l.8-1.9-2 .7M6.6 8.2h.1M9.3 8.2h.1M6.6 10.2c.9.7 1.9.7 2.8 0" />
      </svg>
    );
  }
  return (
    <svg {...COMMON}>
      <path d="M3 12.5V8M8 12.5V3.5M13 12.5V6" />
    </svg>
  );
}
