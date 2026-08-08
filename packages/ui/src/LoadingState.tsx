export interface LoadingStateProps {
  label?: string;
}

export function LoadingState({ label }: LoadingStateProps) {
  return (
    <div className="ui-state" role="status" aria-live="polite">
      {label && <span className="ui-state-title">{label}</span>}
    </div>
  );
}
