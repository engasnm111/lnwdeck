import { useCallback, useEffect, useState } from "react";
import { fetchAnalytics, type AnalyticsRow } from "../../lib/native";
import { DataState, Card, Badge, Button } from "@lnwdeck/ui";

export function AnalyticsPage() {
  const [rows, setRows] = useState<AnalyticsRow[]>([]);
  const [availableProviders, setAvailableProviders] = useState<string[]>([]);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const [providerFilter, setProviderFilter] = useState("");
  const [modelFilter, setModelFilter] = useState("");
  const [confidenceFilter, setConfidenceFilter] = useState("");

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetchAnalytics({
        provider_id: providerFilter || undefined,
        model: modelFilter || undefined,
        confidence: confidenceFilter || undefined,
      });
      setRows(res.rows);
      setAvailableProviders(res.available_providers);
      setAvailableModels(res.available_models);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setLoading(false);
    }
  }, [providerFilter, modelFilter, confidenceFilter]);

  useEffect(() => {
    load();
  }, [load]);

  const totalTokens = rows.reduce(
    (s, r) => s + r.tokens_input + r.tokens_output,
    0
  );
  const totalCost = rows.reduce((s, r) => s + parseFloat(r.cost || "0"), 0);

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
          <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Analytics</h2>
          <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
            Granular event logs and token breakdown
          </p>
        </div>
        <Button variant="secondary" onClick={load} aria-label="Refresh analytics">
          Apply & Refresh
        </Button>
      </div>

      {/* Dynamic Filter Controls */}
      <Card className="mb-4" style={{ marginBottom: "1.5rem" }}>
        <div
          role="region"
          aria-label="Filters"
          style={{
            display: "flex",
            gap: "1.5rem",
            alignItems: "center",
            flexWrap: "wrap",
          }}
        >
          <div>
            <label
              htmlFor="filter-provider"
              style={{
                display: "block",
                fontSize: "0.75rem",
                color: "var(--text-muted)",
                marginBottom: "0.25rem",
              }}
            >
              Provider
            </label>
            <select
              id="filter-provider"
              className="ui-select"
              value={providerFilter}
              onChange={(e) => setProviderFilter(e.target.value)}
            >
              <option value="">All Providers</option>
              {availableProviders.map((p) => (
                <option key={p} value={p}>
                  {p}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label
              htmlFor="filter-model"
              style={{
                display: "block",
                fontSize: "0.75rem",
                color: "var(--text-muted)",
                marginBottom: "0.25rem",
              }}
            >
              Model
            </label>
            <select
              id="filter-model"
              className="ui-select"
              value={modelFilter}
              onChange={(e) => setModelFilter(e.target.value)}
            >
              <option value="">All Models</option>
              {availableModels.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </div>

          <div>
            <label
              htmlFor="filter-confidence"
              style={{
                display: "block",
                fontSize: "0.75rem",
                color: "var(--text-muted)",
                marginBottom: "0.25rem",
              }}
            >
              Confidence
            </label>
            <select
              id="filter-confidence"
              className="ui-select"
              value={confidenceFilter}
              onChange={(e) => setConfidenceFilter(e.target.value)}
            >
              <option value="">All Confidence Levels</option>
              <option value="High">High</option>
              <option value="Medium">Medium</option>
              <option value="Low">Low</option>
            </select>
          </div>
        </div>
      </Card>

      <DataState
        loading={loading}
        error={error}
        isEmpty={rows.length === 0 && !loading}
        emptyFallback={
          <Card title="No Usage Data Found">
            <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
              No usage data yet. Connect a provider to start tracking.
            </p>
            <Badge tone="info">No Records</Badge>
          </Card>
        }
      >
        {/* Summary Card */}
        <div
          role="region"
          aria-label="Summary"
          style={{
            display: "flex",
            gap: "2rem",
            marginBottom: "1.5rem",
            backgroundColor: "var(--bg-panel)",
            padding: "1rem 1.25rem",
            borderRadius: "var(--radius-card)",
            border: "1px solid var(--border-subtle)",
          }}
        >
          <div>
            <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
              Total Filtered Tokens
            </span>
            <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
              {totalTokens.toLocaleString()}
            </p>
          </div>
          <div>
            <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
              Total Filtered Cost
            </span>
            <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
              ${totalCost.toFixed(4)}
            </p>
          </div>
          <div>
            <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
              Recorded Events
            </span>
            <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
              {rows.length}
            </p>
          </div>
        </div>

        {/* Usage Events Data Table */}
        <Card>
          <table className="ui-table" role="table" aria-label="Usage events">
            <thead>
              <tr>
                <th scope="col">Timestamp</th>
                <th scope="col">Provider</th>
                <th scope="col">Model</th>
                <th scope="col">Tokens In</th>
                <th scope="col">Tokens Out</th>
                <th scope="col">Confidence</th>
                <th scope="col">Cost</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id}>
                  <td>{r.timestamp}</td>
                  <td>{r.provider_id}</td>
                  <td>{r.model}</td>
                  <td>{r.tokens_input.toLocaleString()}</td>
                  <td>{r.tokens_output.toLocaleString()}</td>
                  <td>
                    <Badge tone={r.confidence === "High" ? "success" : "warning"}>
                      {r.confidence}
                    </Badge>
                  </td>
                  <td>${parseFloat(r.cost || "0").toFixed(4)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Card>
      </DataState>
    </div>
  );
}
