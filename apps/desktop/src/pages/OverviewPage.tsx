import { useCallback, useEffect, useState } from "react";
import { fetchOverview, OverviewData } from "../lib/native";
import { DataState, MetricCard, Card, Badge, Button } from "@lnwdeck/ui";

export function OverviewPage() {
  const [data, setData] = useState<OverviewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await fetchOverview();
      setData(result);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const isEmpty = data !== null && data.total_events === 0;
  const totalTokens = data
    ? data.total_tokens_input + data.total_tokens_output
    : 0;

  const renderCostBadge = (status: string) => {
    switch (status) {
      case "exact":
        return <Badge tone="success">Exact</Badge>;
      case "estimated":
        return <Badge tone="info">Estimated</Badge>;
      case "missing_pricing":
        return <Badge tone="warning">Missing pricing</Badge>;
      case "no_data":
      default:
        return <Badge tone="default">Unavailable</Badge>;
    }
  };

  return (
    <div>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: "1.5rem",
        }}
      >
        <div>
          <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Overview</h2>
          <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
            Universal AI usage summary across local tools
          </p>
        </div>
        <Button variant="secondary" onClick={load} aria-label="Refresh overview">
          Refresh Overview
        </Button>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={isEmpty}
        emptyFallback={
          <Card title="No Activity Recorded">
            <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
              No supported AI tools were detected or no usage records were found yet.
            </p>
            <Badge tone="warning">No Provider Detected</Badge>
          </Card>
        }
      >
        {data && (
          <div role="region" aria-label="Usage overview">
            {/* Metric Cards Grid */}
            <div className="metrics-grid">
              <MetricCard
                title="Total Tokens"
                value={totalTokens.toLocaleString()}
                subtitle={`In: ${data.total_tokens_input.toLocaleString()} | Out: ${data.total_tokens_output.toLocaleString()}`}
                badge={<Badge tone="info">Exact</Badge>}
              />
              <MetricCard
                title="Total Cost"
                value={data.cost_formatted || (data.total_cost > 0 ? `$${data.total_cost.toFixed(4)}` : "$0.00")}
                subtitle={data.cost_status === "missing_pricing" ? "Pricing catalog incomplete" : "Calculated from pricing tables"}
                badge={renderCostBadge(data.cost_status)}
              />
              <MetricCard
                title="Requests / Events"
                value={data.total_events.toLocaleString()}
                subtitle={`${data.provider_count} active providers`}
                badge={<Badge tone="success">Active</Badge>}
              />
              <MetricCard
                title="Budget Status"
                value="Under Limit"
                subtitle="No active threshold breaches"
                badge={<Badge tone="success">OK</Badge>}
              />
            </div>

            {/* Main Overview Panels */}
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "2fr 1fr",
                gap: "1.5rem",
              }}
            >
              <Card title="Token Usage Over Time">
                <div
                  style={{
                    height: "180px",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    backgroundColor: "var(--bg-panel-elevated)",
                    borderRadius: "var(--radius-card)",
                    border: "1px dashed var(--border-strong)",
                    color: "var(--text-muted)",
                  }}
                >
                  <p>
                    {data.total_events > 0
                      ? `${data.total_events} events tracked from ${data.oldest_event_at || "earliest"} to ${data.latest_event_at || "now"}`
                      : "No timeline data"}
                  </p>
                </div>
              </Card>

              <Card title="Data Freshness & Confidence">
                <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
                  <div>
                    <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                      High Confidence Events
                    </span>
                    <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
                      {data.high_confidence_count} / {data.total_events}
                    </p>
                  </div>
                  <div>
                    <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                      Confidence Coverage
                    </span>
                    <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
                      {(data.confidence_coverage * 100).toFixed(1)}%
                    </p>
                  </div>
                  <div>
                    <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
                      Latest Event Timestamp
                    </span>
                    <p style={{ fontSize: "0.875rem", color: "var(--text-secondary)" }}>
                      {data.latest_event_at || "—"}
                    </p>
                  </div>
                </div>
              </Card>
            </div>
          </div>
        )}
      </DataState>
    </div>
  );
}
