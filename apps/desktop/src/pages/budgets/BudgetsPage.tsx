import { Card, Badge } from "@lnwdeck/ui";

export function BudgetsPage() {
  return (
    <div>
      <div style={{ marginBottom: "1.5rem" }}>
        <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Budgets</h2>
        <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
          Spending targets, token limits, and alert thresholds
        </p>
      </div>

      <Card title="Budget Tracking & Thresholds">
        <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
          No custom budget limits have been configured for v0.1. Default threshold monitoring is active.
        </p>
        <Badge tone="success">Under Limit</Badge>
      </Card>
    </div>
  );
}
