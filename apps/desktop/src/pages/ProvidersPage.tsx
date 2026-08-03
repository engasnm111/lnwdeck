import { DataState } from "@lnwdeck/ui";

export function ProvidersPage() {
  return (
    <div>
      <h2>Providers</h2>
      <DataState loading={false} error={null} isEmpty={true}>
        <p>Provider data will appear here.</p>
      </DataState>
    </div>
  );
}
