import { DataState } from "@inwdeck/ui";

export function BudgetsPage() {
  return (
    <div>
      <h2>Budgets</h2>
      <DataState loading={false} error={null} isEmpty={true}>
        <p>Budget tracking will appear here.</p>
      </DataState>
    </div>
  );
}
