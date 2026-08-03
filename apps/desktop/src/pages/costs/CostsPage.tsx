import { Card, Badge } from "@lnwdeck/ui";

export function CostsPage() {
  return (
    <div>
      <div style={{ marginBottom: "1.5rem" }}>
        <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Costs</h2>
        <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
          Cost calculation per model, token tier, and provider pricing
        </p>
      </div>

      <Card title="Cost Breakdown & Pricing Status">
        <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
          Calculated cost estimates are based on local token counters and open model pricing tables.
        </p>
        <div style={{ display: "flex", gap: "0.5rem" }}>
          <Badge tone="info">Estimated</Badge>
          <Badge tone="success">Local Pricing</Badge>
        </div>
      </Card>
    </div>
  );
}
