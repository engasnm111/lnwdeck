import { DataState } from "@inwdeck/ui";

export function SystemPage() {
  return (
    <div>
      <h2>System</h2>
      <DataState loading={false} error={null} isEmpty={false}>
        <div role="region" aria-label="Diagnostics">
          <h3>Database</h3>
          <table>
            <tbody>
              <tr><th>Integrity</th><td>Unknown</td></tr>
              <tr><th>Size</th><td>—</td></tr>
              <tr><th>Events</th><td>—</td></tr>
            </tbody>
          </table>
        </div>

        <div role="region" aria-label="Data Management" style={{ marginTop: "1.5rem" }}>
          <h3>Data Management</h3>
          <button type="button" aria-label="Delete all data">
            Delete All Data
          </button>
        </div>
      </DataState>
    </div>
  );
}
