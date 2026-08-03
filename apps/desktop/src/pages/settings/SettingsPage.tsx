import { DataState } from "@inwdeck/ui";

export function SettingsPage() {
  return (
    <div>
      <h2>Settings</h2>
      <DataState loading={false} error={null} isEmpty={false}>
        <form role="form" aria-label="Application settings">
          <fieldset>
            <legend>Startup</legend>
            <label>
              <input type="checkbox" defaultChecked />
              Launch at startup
            </label>
          </fieldset>

          <fieldset>
            <legend>Appearance</legend>
            <label htmlFor="theme">Theme</label>
            <select id="theme">
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </fieldset>

          <fieldset>
            <legend>Data Refresh</legend>
            <label htmlFor="refresh-interval">Auto-refresh interval</label>
            <select id="refresh-interval">
              <option value="30">30 seconds</option>
              <option value="60">1 minute</option>
              <option value="300">5 minutes</option>
              <option value="0">Off</option>
            </select>
          </fieldset>

          <fieldset>
            <legend>Updates</legend>
            <label>
              <input type="checkbox" defaultChecked />
              Check for updates automatically
            </label>
          </fieldset>
        </form>
      </DataState>
    </div>
  );
}
