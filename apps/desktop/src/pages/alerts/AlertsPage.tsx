import { DataState } from "@inwdeck/ui";

export function AlertsPage() {
  return (
    <div>
      <h2>Alerts</h2>
      <DataState loading={false} error={null} isEmpty={true}>
        <p>Alerts configuration will appear here.</p>
      </DataState>
    </div>
  );
}
