export interface LoadingStateProps {
  label?: string;
}

export function LoadingState({ label = "Loading" }: LoadingStateProps) {
  return (
    <div className="ui-state" role="status" aria-live="polite">
      <span className="ui-state-title">{label}</span>
    </div>
  );
}
