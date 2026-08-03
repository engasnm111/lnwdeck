export function EmptyState({ message = "No data available" }: { message?: string }) {
  return (
    <div role="status">
      <p>{message}</p>
    </div>
  );
}
