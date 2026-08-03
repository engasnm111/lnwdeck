import { useCallback, useEffect, useState } from "react";
import { DataState } from "@lnwdeck/ui";
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
  if (!value) {
    return "—";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "—";
  }
  return date.toLocaleString();
}

function healthLabel(row: ProviderRow): { label: string; tone: "ok" | "warn" | "error" } {
  const { provider, run } = row;
  if (run && run.error_code) {
    return { label: "Error", tone: "error" };
  }
  if (provider.detected) {
    return { label: "Detected", tone: "ok" };
  }
  if (provider.source_exists) {
    return { label: "Unreadable", tone: "warn" };
  }
  return { label: "Not detected", tone: "warn" };
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
      <h2>System</h2>
      <DataState
        loading={loading}
        error={error}
        isEmpty={false}
        errorFallback={
          <p role="alert">
            Failed to read pipeline diagnostics: {error?.message}
          </p>
        }
      >
        {diagnostics && (
          <div role="region" aria-label="Data Pipeline">
            <h3>Data Pipeline</h3>

            <dl aria-label="Database status">
              <div>
                <dt>App version</dt>
                <dd>{diagnostics.app_version}</dd>
              </div>
              <div>
                <dt>Database</dt>
                <dd>
                  {diagnostics.db_ok && diagnostics.integrity_ok
                    ? "Healthy"
                    : "Error"}
                </dd>
              </div>
              <div>
                <dt>Migration version</dt>
                <dd>{diagnostics.migration_version}</dd>
              </div>
              <div>
                <dt>Events stored</dt>
                <dd>{diagnostics.total_events}</dd>
              </div>
              <div>
                <dt>Last successful sync</dt>
                <dd>{formatTimestamp(diagnostics.totals.last_successful_sync)}</dd>
              </div>
              <div>
                <dt>Next retry</dt>
                <dd>{formatTimestamp(diagnostics.totals.next_retry_at)}</dd>
              </div>
              <div>
                <dt>Privacy rejections</dt>
                <dd>{diagnostics.totals.privacy_rejections}</dd>
              </div>
            </dl>

            <div aria-label="Actions" style={{ display: "flex", gap: "0.5rem" }}>
              <button
                type="button"
                onClick={handleRefresh}
                disabled={refreshing}
                aria-label="Refresh all providers"
              >
                {refreshing ? "Refreshing…" : "Refresh All"}
              </button>
              <button
                type="button"
                onClick={() => setExportOpen((open) => !open)}
                aria-label="Export sanitized diagnostics"
              >
                Export Sanitized Diagnostics
              </button>
            </div>

            {noProviders && (
              <p>
                No supported AI tools were detected. Scan again or configure a
                provider manually.
              </p>
            )}
            {detectedButNoRecords && (
              <p>
                Provider detected, but no usage records were found yet. Open
                provider diagnostics for collection details.
              </p>
            )}
            {rows.length > 0 && (
              <table aria-label="Provider collection health">
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
                          <span data-tone={health.tone}>{health.label}</span>
                          {run?.error_code ? (
                            <small> ({run.error_code})</small>
                          ) : null}
                        </td>
                        <td>{formatTimestamp(run?.next_retry_at)}</td>
                        <td>
                          <button
                            type="button"
                            onClick={handleRefresh}
                            disabled={refreshing}
                            aria-label={`Refresh ${row.provider.display_name}`}
                          >
                            Refresh
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}

            {exportOpen && diagnostics && (
              <pre
                role="region"
                aria-label="Exported diagnostics"
                data-testid="exported-diagnostics"
              >
                {JSON.stringify(diagnostics, null, 2)}
              </pre>
            )}
          </div>
        )}

        <div role="region" aria-label="Data Management" style={{ marginTop: "1.5rem" }}>
          <h3>Data Management</h3>
          <button type="button" aria-label="Delete all data">
            Delete All Data
          </button>
        </div>
      </DataState>
    </div>
  );
}
