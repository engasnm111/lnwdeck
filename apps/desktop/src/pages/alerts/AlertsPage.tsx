import { Card, Badge } from "@lnwdeck/ui";

export function AlertsPage() {
  return (
    <div>
      <div style={{ marginBottom: "1.5rem" }}>
        <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Alerts</h2>
        <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
          Usage limit warnings, quota thresholds, and collection error alerts
        </p>
      </div>

      <Card title="Recent Alerts & Notifications">
        <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
          No threshold breaches or provider errors detected in the current window.
        </p>
        <Badge tone="success">All Systems Normal</Badge>
      </Card>
    </div>
  );
}
