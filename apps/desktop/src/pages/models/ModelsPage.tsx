import { Card, Badge } from "@lnwdeck/ui";

export function ModelsPage() {
  return (
    <div>
      <div style={{ marginBottom: "1.5rem" }}>
        <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Models</h2>
        <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
          Model performance, token distribution, and request counts
        </p>
      </div>

      <Card title="Model Usage Distribution">
        <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
          Model usage analytics automatically aggregates model names from ingested local events.
        </p>
        <Badge tone="info">Active Ingestion</Badge>
      </Card>
    </div>
  );
}
