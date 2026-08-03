export function LoadingState({ message = "Loading..." }: { message?: string }) {
  return (
    <div role="status" aria-live="polite">
      <p>{message}</p>
    </div>
  );
}
