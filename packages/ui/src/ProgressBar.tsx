export interface ProgressBarProps {
  /**
   * Percentage to fill, 0..100. `null` means the value is unknown: the track
   * renders hatched and no fill is drawn, so an unknown limit can never look
   * like a full or an empty bar.
   */
  percent: number | null;
  tone?: "success" | "warning" | "danger" | "accent";
  label: string;
}

export function ProgressBar({
  percent,
  tone = "accent",
  label,
}: ProgressBarProps) {
  if (percent === null) {
    return (
      <div
        className="ui-progress ui-progress-unknown"
        role="img"
        aria-label={`${label}: no limit reported`}
      />
    );
  }
  const clamped = Math.max(0, Math.min(100, percent));
  return (
    <div
      className="ui-progress"
      role="progressbar"
      aria-label={label}
      aria-valuenow={Math.round(clamped)}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div
        className={`ui-progress-fill ui-progress-fill-${tone}`}
        style={{ width: `${clamped}%` }}
      />
    </div>
  );
}
