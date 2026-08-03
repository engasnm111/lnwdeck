import { DataState } from "@lnwdeck/ui";

export function ModelsPage() {
  return (
    <div>
      <h2>Models</h2>
      <DataState loading={false} error={null} isEmpty={true}>
        <p>Model analytics will appear here.</p>
      </DataState>
    </div>
  );
}
