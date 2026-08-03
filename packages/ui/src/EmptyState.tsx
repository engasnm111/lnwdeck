export interface EmptyStateProps {
  title?: string;
  detail?: string;
}

/**
 * Shown when a query succeeded and returned nothing. The wording states that
 * there is no data rather than implying everything is fine.
 */
export function EmptyState({
  title = "No data yet",
  detail = "Nothing has been recorded for this view.",
}: EmptyStateProps) {
  return (
    <div className="ui-state" role="status">
      <span className="ui-state-title">{title}</span>
      <span className="ui-state-detail">{detail}</span>
    </div>
  );
}
