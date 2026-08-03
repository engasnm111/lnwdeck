import { DataState } from "@lnwdeck/ui";

export function CostsPage() {
  return (
    <div>
      <h2>Costs</h2>
      <DataState loading={false} error={null} isEmpty={true}>
        <p>Cost breakdown will appear here.</p>
      </DataState>
    </div>
  );
}
