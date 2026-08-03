import { useCallback, useEffect, useState } from "react";
import { fetchProviders, refreshAll, type DetailedProviderInfo } from "../lib/native";
import { DataState, Card, Badge, Button } from "@lnwdeck/ui";

export function ProvidersPage() {
  const [providers, setProviders] = useState<DetailedProviderInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await fetchProviders();
      setProviders(list);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    try {
      await refreshAll();
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setRefreshing(false);
      await load();
    }
  }, [load]);

  const isEmpty = providers.length === 0 && !loading;

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
          <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>Providers</h2>
          <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
            Local AI engine adapters and collection state
          </p>
        </div>
        <Button
          variant="secondary"
          onClick={handleRefresh}
          disabled={refreshing}
          aria-label="Refresh providers"
        >
          {refreshing ? "Refreshing…" : "Scan & Refresh Providers"}
        </Button>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={isEmpty}
        emptyFallback={
          <Card title="No Supported AI Tools Detected">
            <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
              No supported AI tools were detected on your system.
            </p>
            <Badge tone="warning">No Provider Detected</Badge>
          </Card>
        }
      >
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))", gap: "1.5rem" }}>
          {providers.map((p) => (
            <Card key={p.provider_id} title={p.display_name}>
              <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Status</span>
                  <Badge tone={p.detected ? "success" : "warning"}>
                    {p.detected ? "Detected" : "Not Detected"}
                  </Badge>
                </div>

                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Source</span>
                  <span style={{ fontSize: "0.875rem" }}>{p.source_type}</span>
                </div>

                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Health</span>
                  <Badge tone={p.health_status.includes("Error") ? "danger" : "success"}>
                    {p.health_status}
                  </Badge>
                </div>

                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Event Count</span>
                  <span style={{ fontSize: "0.875rem", fontWeight: 600 }}>{p.event_count}</span>
                </div>

                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Total Tokens</span>
                  <span style={{ fontSize: "0.875rem", fontWeight: 600 }}>{p.total_tokens.toLocaleString()}</span>
                </div>

                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Last Sync</span>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-secondary)" }}>
                    {p.last_sync ? new Date(p.last_sync).toLocaleString() : "—"}
                  </span>
                </div>

                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Quota Window</span>
                  <span style={{ fontSize: "0.875rem" }}>{p.quota_summary}</span>
                </div>

                <div style={{ marginTop: "0.5rem" }}>
                  <Button variant="secondary" onClick={handleRefresh} disabled={refreshing} style={{ width: "100%" }}>
                    Refresh Adapter
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      </DataState>
    </div>
  );
}
