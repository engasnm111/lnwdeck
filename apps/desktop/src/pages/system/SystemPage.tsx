import { useCallback, useEffect, useState } from "react";
import { DataState, Card, Badge, Button } from "@lnwdeck/ui";
import {
  fetchPipelineDiagnostics,
  refreshAll,
  type CollectorRunRow,
  type PipelineDiagnostics,
  type ProviderStateRow,
} from "../../lib/native";

interface ProviderRow {
  provider: ProviderStateRow;
  run: CollectorRunRow | undefined;
}

function joinRows(diagnostics: PipelineDiagnostics): ProviderRow[] {
  return diagnostics.providers.map((provider) => ({
    provider,
    run: diagnostics.runs.find((run) => run.provider_id === provider.provider_id),
  }));
}

function formatTimestamp(value: string | null | undefined): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return date.toLocaleString();
}

function healthLabel(row: ProviderRow): { label: string; tone: "success" | "warning" | "danger" } {
  const { provider, run } = row;
  if (run && run.error_code) {
    return { label: "Error", tone: "danger" };
  }
  if (provider.detected) {
    return { label: "Detected", tone: "success" };
  }
  if (provider.source_exists) {
    return { label: "Unreadable", tone: "warning" };
  }
  return { label: "Not detected", tone: "warning" };
}

export function SystemPage() {
  const [diagnostics, setDiagnostics] = useState<PipelineDiagnostics | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [exportOpen, setExportOpen] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await fetchPipelineDiagnostics();
      setDiagnostics(result);
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

  const rows = diagnostics ? joinRows(diagnostics) : [];
  const noProviders = diagnostics !== null && diagnostics.providers.length === 0;
  const detectedButNoRecords =
    diagnostics !== null &&
    !noProviders &&
    diagnostics.runs.every((run) => run.events_inserted === 0 && !run.error_code);

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
          <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>System</h2>
          <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
            Data pipeline diagnostics, storage health, and audit logs
          </p>
        </div>
        <div style={{ display: "flex", gap: "0.75rem" }}>
          <Button
            variant="secondary"
            onClick={handleRefresh}
            disabled={refreshing}
            aria-label="Refresh all providers"
          >
            {refreshing ? "Refreshing…" : "Refresh All"}
          </Button>
          <Button
            variant="secondary"
            onClick={() => setExportOpen((open) => !open)}
            aria-label="Export sanitized diagnostics"
          >
            Export Sanitized Diagnostics
          </Button>
        </div>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={false}
        errorFallback={
          <Card title="Diagnostics Error">
            <p role="alert" style={{ color: "var(--danger)" }}>
              Failed to read pipeline diagnostics: {error?.message}
            </p>
          </Card>
        }
      >
        {diagnostics && (
          <div role="region" aria-label="Data Pipeline" style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
            <h3 role="heading" style={{ fontSize: "1.25rem", fontWeight: 600 }}>Data Pipeline</h3>
            {/* Database & Diagnostics Cards */}
            <Card title="Database & Storage Health">
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
                  gap: "1rem",
                }}
              >
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>App Version</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.app_version}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Database Status</span>
                  <p>
                    <Badge tone={diagnostics.db_ok && diagnostics.integrity_ok ? "success" : "danger"}>
                      {diagnostics.db_ok && diagnostics.integrity_ok ? "Healthy" : "Degraded"}
                    </Badge>
                  </p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Migration version</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.migration_version}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Events stored</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.total_events}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Privacy Rejections</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.totals.privacy_rejections}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>Last Sync</span>
                  <p style={{ fontSize: "0.875rem" }}>{formatTimestamp(diagnostics.totals.last_successful_sync)}</p>
                </div>
              </div>
            </Card>

            {noProviders && (
              <Card title="No Providers Detected">
                <p style={{ color: "var(--text-secondary)" }}>
                  No supported AI tools were detected. Scan again or configure a provider manually.
                </p>
              </Card>
            )}

            {detectedButNoRecords && (
              <Card title="Pending Data">
                <p style={{ color: "var(--text-secondary)" }}>
                  Provider detected, but no usage records were found yet. Open provider diagnostics for collection details.
                </p>
              </Card>
            )}

            {/* Provider Collection Table */}
            {rows.length > 0 && (
              <Card title="Provider Pipeline Diagnostics">
                <table className="ui-table" aria-label="Provider collection health">
                  <thead>
                    <tr>
                      <th scope="col">Provider</th>
                      <th scope="col">Detected</th>
                      <th scope="col">Source</th>
                      <th scope="col">Mode</th>
                      <th scope="col">Last sync</th>
                      <th scope="col">Seen</th>
                      <th scope="col">Parsed</th>
                      <th scope="col">Inserted</th>
                      <th scope="col">Duplicates</th>
                      <th scope="col">Rejected</th>
                      <th scope="col">Health</th>
                      <th scope="col">Next retry</th>
                      <th scope="col">Action</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => {
                      const health = healthLabel(row);
                      const run = row.run;
                      return (
                        <tr key={row.provider.provider_id}>
                          <td>{row.provider.display_name}</td>
                          <td>{row.provider.detected ? "Yes" : "No"}</td>
                          <td>{row.provider.source_type || "—"}</td>
                          <td>{run?.collector_mode ?? "—"}</td>
                          <td>{formatTimestamp(run?.finished_at)}</td>
                          <td>{run?.source_records_seen ?? 0}</td>
                          <td>{run?.records_parsed ?? 0}</td>
                          <td>{run?.events_inserted ?? 0}</td>
                          <td>{run?.duplicates_skipped ?? 0}</td>
                          <td>{run?.events_rejected ?? 0}</td>
                          <td>
                            <Badge tone={health.tone}>{health.label}</Badge>
                            {run?.error_code ? <small> ({run.error_code})</small> : null}
                          </td>
                          <td>{formatTimestamp(run?.next_retry_at)}</td>
                          <td>
                            <Button
                              variant="secondary"
                              onClick={handleRefresh}
                              disabled={refreshing}
                              aria-label={`Refresh ${row.provider.display_name}`}
                              style={{ padding: "0.25rem 0.5rem", fontSize: "0.75rem" }}
                            >
                              Refresh
                            </Button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </Card>
            )}

            {/* Export Diagnostics Panel */}
            {exportOpen && diagnostics && (
              <Card title="Sanitized Diagnostics Output">
                <pre
                  role="region"
                  aria-label="Exported diagnostics"
                  data-testid="exported-diagnostics"
                  style={{
                    backgroundColor: "var(--bg-app)",
                    padding: "1rem",
                    borderRadius: "var(--radius-control)",
                    overflowX: "auto",
                    color: "var(--text-secondary)",
                    fontSize: "0.8125rem",
                  }}
                >
                  {JSON.stringify(diagnostics, null, 2)}
                </pre>
              </Card>
            )}
          </div>
        )}
      </DataState>
    </div>
  );
}
