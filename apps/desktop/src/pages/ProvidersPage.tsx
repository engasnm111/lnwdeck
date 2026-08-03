import { useCallback, useEffect, useState } from "react";
import { fetchProviders, refreshAll, refreshProvider, type DetailedProviderInfo } from "../lib/native";
import { DataState, Card, Badge, Button } from "@lnwdeck/ui";

export function ProvidersPage() {
  const [providers, setProviders] = useState<DetailedProviderInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
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

  const handleRefreshProvider = useCallback(
    async (providerId: string) => {
      setRefreshingId(providerId);
      try {
        await refreshProvider(providerId);
      } catch (e) {
        setError(e instanceof Error ? e : new Error(String(e)));
      } finally {
        setRefreshingId(null);
        await load();
      }
    },
    [load],
  );

  const renderStatusBadge = (p: DetailedProviderInfo) => {
    if (p.health_status.startsWith("Error")) {
      return <Badge tone="danger">{p.health_status}</Badge>;
    }
    if (p.event_count > 0) {
      return <Badge tone="success">Active</Badge>;
    }
    if (p.detected) {
      return <Badge tone="info">Detected (No events)</Badge>;
    }
    if (p.health_status === "Permission required") {
      return <Badge tone="warning">Permission required</Badge>;
    }
    return <Badge tone="default">Not configured</Badge>;
  };

  const renderCostBadge = (support: string) => {
    switch (support) {
      case "Exact":
        return <Badge tone="success">Exact Pricing</Badge>;
      case "Estimated":
        return <Badge tone="info">Estimated Pricing</Badge>;
      case "Free / Local":
        return <Badge tone="success">Free / Local</Badge>;
      default:
        return <Badge tone="default">Pricing N/A</Badge>;
    }
  };

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
            Local AI engine adapters and collection state (Codex, Gemini, Kiro, Claude, OpenCode & more)
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
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Status</span>
                  {renderStatusBadge(p)}
                </div>

                <div style={{ display: "flex", justifyContent: "space-between" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Source</span>
                  <span style={{ fontSize: "0.875rem" }}>{p.source_type}</span>
                </div>

                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Cost Support</span>
                  {renderCostBadge(p.cost_support || "Exact")}
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
                  <Button
                    variant="secondary"
                    onClick={() => void handleRefreshProvider(p.provider_id)}
                    disabled={refreshingId === p.provider_id}
                    style={{ width: "100%" }}
                  >
                    {refreshingId === p.provider_id
                      ? "Refreshing…"
                      : "Refresh Adapter"}
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
