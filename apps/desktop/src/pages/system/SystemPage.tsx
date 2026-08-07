import { useCallback, useEffect, useState } from "react";
import { DataState, Card, Badge, Button } from "@lnwdeck/ui";
import {
  fetchPipelineDiagnostics,
  refreshAll,
  type CollectorRunRow,
  type PipelineDiagnostics,
  type ProviderStateRow,
} from "../../lib/native";
import { useI18n } from "../../lib/i18n";

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

function formatTimestamp(value: string | null | undefined, locale = "en-US"): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(locale, {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date);
}

function healthLabel(row: ProviderRow, t: (key: string) => string): { label: string; tone: "success" | "warning" | "danger" } {
  const { provider, run } = row;
  if (run && run.error_code) {
    return { label: t("system.health.error"), tone: "danger" };
  }
  if (provider.detected) {
    return { label: t("system.health.detected"), tone: "success" };
  }
  if (provider.source_exists) {
    return { label: t("system.health.unreadable"), tone: "warning" };
  }
  return { label: t("system.health.notDetected"), tone: "warning" };
}

export function SystemPage() {
  const { t, language } = useI18n();
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
          <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>{t("nav.system")}</h2>
          <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
            {t("system.subtitle")}
          </p>
        </div>
        <div style={{ display: "flex", gap: "0.75rem" }}>
          <Button
            variant="secondary"
            onClick={handleRefresh}
            disabled={refreshing}
            aria-label={t("topbar.refresh")}
          >
            {refreshing ? t("topbar.refreshing") : t("system.refreshAll")}
          </Button>
          <Button
            variant="secondary"
            onClick={() => setExportOpen((open) => !open)}
            aria-label={t("system.exportAria")}
          >
            {t("system.export")}
          </Button>
        </div>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={false}
        errorFallback={
          <Card title={t("system.error.title")}>
            <p role="alert" style={{ color: "var(--danger)" }}>
              {t("system.error.body", { error: error?.message ?? "" })}
            </p>
          </Card>
        }
      >
        {diagnostics && (
          <div role="region" aria-label={t("system.pipeline")} style={{ display: "flex", flexDirection: "column", gap: "1.5rem" }}>
            <h3 role="heading" style={{ fontSize: "1.25rem", fontWeight: 600 }}>{t("system.pipeline")}</h3>
            {/* Database & Diagnostics Cards */}
            <Card title={t("system.db.title")}>
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
                  gap: "1rem",
                }}
              >
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>{t("system.db.appVersion")}</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.app_version}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>{t("system.db.status")}</span>
                  <p>
                    <Badge tone={diagnostics.db_ok && diagnostics.integrity_ok ? "success" : "danger"}>
                      {diagnostics.db_ok && diagnostics.integrity_ok ? t("system.db.healthy") : t("system.db.degraded")}
                    </Badge>
                  </p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>{t("system.db.migrationVersion")}</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.migration_version}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>{t("system.db.eventsStored")}</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.total_events}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>{t("system.db.privacyRejections")}</span>
                  <p style={{ fontWeight: 600 }}>{diagnostics.totals.privacy_rejections}</p>
                </div>
                <div>
                  <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>{t("system.db.lastSync")}</span>
                  <p style={{ fontSize: "0.875rem" }}>{formatTimestamp(diagnostics.totals.last_successful_sync, language)}</p>
                </div>
              </div>
            </Card>

            {noProviders && (
              <Card title={t("system.noProviders.title")}>
                <p style={{ color: "var(--text-secondary)" }}>
                  {t("system.noProviders.body")}
                </p>
              </Card>
            )}

            {detectedButNoRecords && (
              <Card title={t("system.pending.title")}>
                <p style={{ color: "var(--text-secondary)" }}>
                  {t("system.pending.body")}
                </p>
              </Card>
            )}

            {/* Provider Collection Table */}
            {rows.length > 0 && (
              <Card title={t("system.table.title")}>
                <table className="ui-table" aria-label={t("system.table.aria")}>
                  <thead>
                    <tr>
                      <th scope="col">{t("system.table.provider")}</th>
                      <th scope="col">{t("system.table.detected")}</th>
                      <th scope="col">{t("system.table.source")}</th>
                      <th scope="col">{t("system.table.mode")}</th>
                      <th scope="col">{t("system.table.lastSync")}</th>
                      <th scope="col">{t("system.table.seen")}</th>
                      <th scope="col">{t("system.table.parsed")}</th>
                      <th scope="col">{t("system.table.inserted")}</th>
                      <th scope="col">{t("system.table.duplicates")}</th>
                      <th scope="col">{t("system.table.rejected")}</th>
                      <th scope="col">{t("system.table.health")}</th>
                      <th scope="col">{t("system.table.nextRetry")}</th>
                      <th scope="col">{t("system.table.action")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((row) => {
                      const health = healthLabel(row, t);
                      const run = row.run;
                      return (
                        <tr key={row.provider.provider_id}>
                          <td>{row.provider.display_name}</td>
                          <td>{row.provider.detected ? t("common.yes") : t("common.no")}</td>
                          <td>{row.provider.source_type || "—"}</td>
                          <td>{run?.collector_mode ?? "—"}</td>
                          <td>{formatTimestamp(run?.finished_at, language)}</td>
                          <td>{run?.source_records_seen ?? 0}</td>
                          <td>{run?.records_parsed ?? 0}</td>
                          <td>{run?.events_inserted ?? 0}</td>
                          <td>{run?.duplicates_skipped ?? 0}</td>
                          <td>{run?.events_rejected ?? 0}</td>
                          <td>
                            <Badge tone={health.tone}>{health.label}</Badge>
                            {run?.error_code ? <small> ({run.error_code})</small> : null}
                          </td>
                          <td>{formatTimestamp(run?.next_retry_at, language)}</td>
                          <td>
                            <Button
                              variant="secondary"
                              onClick={handleRefresh}
                              disabled={refreshing}
                              aria-label={t("system.refreshProvider", { provider: row.provider.display_name })}
                              style={{ padding: "0.25rem 0.5rem", fontSize: "0.75rem" }}
                            >
                              {t("system.refresh")}
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
              <Card title={t("system.export.title")}>
                <pre
                  role="region"
                  aria-label={t("system.export.aria")}
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
