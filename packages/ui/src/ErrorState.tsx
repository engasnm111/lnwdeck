export interface ErrorStateProps {
  error: Error;
  onRetry?: () => void;
  title?: string;
  retryLabel?: string;
}

/** Shows the real backend failure. It never substitutes data. */
export function ErrorState({ error, onRetry, title, retryLabel }: ErrorStateProps) {
  return (
    <div className="ui-state ui-state-error" role="alert">
      {title && <span className="ui-state-title">{title}</span>}
      <span className="ui-state-detail">{error.message}</span>
      {onRetry && (
        <button type="button" className="ui-button" onClick={onRetry}>
          {retryLabel}
        </button>
      )}
    </div>
  );
}
