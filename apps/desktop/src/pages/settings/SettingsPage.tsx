import { Card, Badge, Button } from "@lnwdeck/ui";

export function SettingsPage() {
  return (
    <div>
      <div style={{ marginBottom: "1.5rem" }}>
        <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Settings</h2>
        <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
          Application preferences, theme, and collection intervals
        </p>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
        <Card title="Startup & System Integration">
          <form role="form" aria-label="Application settings" style={{ display: "flex", flexDirection: "column", gap: "1.25rem" }}>
            <div>
              <label style={{ display: "flex", alignItems: "center", gap: "0.5rem", cursor: "pointer" }}>
                <input type="checkbox" defaultChecked />
                <span>Launch lnwdeck automatically when Windows starts</span>
              </label>
            </div>

            <div>
              <label htmlFor="theme" style={{ display: "block", fontSize: "0.75rem", color: "var(--text-muted)", marginBottom: "0.25rem" }}>
                Theme
              </label>
              <select id="theme" className="ui-select" style={{ width: "200px" }}>
                <option value="dark">Dark Theme (Default)</option>
                <option value="light">Light Theme</option>
                <option value="system">Follow System</option>
              </select>
            </div>

            <div>
              <label htmlFor="refresh-interval" style={{ display: "block", fontSize: "0.75rem", color: "var(--text-muted)", marginBottom: "0.25rem" }}>
                Auto-refresh interval
              </label>
              <select id="refresh-interval" className="ui-select" style={{ width: "200px" }}>
                <option value="300">5 minutes (Default)</option>
                <option value="60">1 minute</option>
                <option value="30">30 seconds</option>
                <option value="0">Disabled</option>
              </select>
            </div>

            <div>
              <label style={{ display: "flex", alignItems: "center", gap: "0.5rem", cursor: "pointer" }}>
                <input type="checkbox" defaultChecked />
                <span>Check for updates automatically</span>
              </label>
            </div>

            <div style={{ marginTop: "0.5rem" }}>
              <Button variant="primary" type="button">Save Settings</Button>
            </div>
          </form>
        </Card>

        <Card title="Privacy & Security Guarantee">
          <p style={{ color: "var(--text-secondary)", marginBottom: "0.75rem" }}>
            lnwdeck is strictly local-only. Prompts, responses, source code, file contents, and credentials are never collected or stored.
          </p>
          <Badge tone="success">Local Metadata Only</Badge>
        </Card>
      </div>
    </div>
  );
}
