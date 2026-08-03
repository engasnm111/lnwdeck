export interface ErrorStateProps {
  error: Error;
  onRetry?: () => void;
}

/** Shows the real backend failure. It never substitutes data. */
export function ErrorState({ error, onRetry }: ErrorStateProps) {
  return (
    <div className="ui-state ui-state-error" role="alert">
      <span className="ui-state-title">Could not load this view</span>
      <span className="ui-state-detail">{error.message}</span>
      {onRetry && (
        <button type="button" className="ui-button" onClick={onRetry}>
          Try again
        </button>
      )}
    </div>
  );
}
