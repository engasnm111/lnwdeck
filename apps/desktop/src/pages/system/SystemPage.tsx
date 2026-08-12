import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { DataState, Card, Badge, Button } from "@lnwdeck/ui";
import {
  fetchPipelineDiagnostics,
  startRefresh,
  type RefreshProgressEvent,
  type CollectorRunRow,
  type PipelineDiagnostics,
  type ProviderStateRow,
} from "../../lib/native";
import { useAsyncLoad } from "../../lib/use-page-load";
import { dataStateLabels, useI18n } from "../../lib/i18n";
import { providerDisplayName } from "../../components/ProviderLogo";
import { providerSourceLabel } from "../../lib/providerText";

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
  const [refreshing, setRefreshing] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportPath, setExportPath] = useState<string | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  const { loading, error } = useAsyncLoad(
    async (_background, isCurrent) => {
      const result = await fetchPipelineDiagnostics();
      if (isCurrent()) {
        setDiagnostics(result);
      }
    },
    [],
    { listenRefreshProgress: true },
  );

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void listen<RefreshProgressEvent>("refresh-progress", (event) => {
      const progress = event.payload;
      if (progress.phase === "started" || progress.phase === "progress") {
        setRefreshing(true);
        return;
      }
      setRefreshing(false);
    }).then((cleanup) => {
      unlisten = cleanup;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const handleExport = useCallback(async () => {
    setExporting(true);
    setExportError(null);
    setExportPath(null);
    try {
      const { exportDiagnostics } = await import("../../lib/native");
      const path = await exportDiagnostics();
      setExportPath(path);
    } catch (e) {
      setExportError(e instanceof Error ? e.message : String(e));
    } finally {
      setExporting(false);
    }
  }, []);

  const handleReveal = useCallback(async () => {
    if (!exportPath) return;
    try {
      const { revealInExplorer } = await import("../../lib/native");
      await revealInExplorer(exportPath);
    } catch {
      // The file was still written; the path stays visible for manual access.
    }
  }, [exportPath]);

  const [refreshError, setRefreshError] = useState<string | null>(null);

  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    setRefreshError(null);
    try {
      const result = await startRefresh();
      if (!result.started && !result.already_running) {
        setRefreshing(false);
      }
    } catch (e) {
      setRefreshError(e instanceof Error ? e.message : String(e));
      setRefreshing(false);
    }
  }, []);

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
            onClick={() => void handleExport()}
            disabled={exporting}
            aria-label={t("system.exportAria")}
          >
            {exporting ? t("common.saving") : t("system.export")}
          </Button>
        </div>
      </div>

      {refreshError && (
        <p className="ui-field-error" role="alert" style={{ marginBottom: "1rem" }}>
          {t("system.error.body", { error: refreshError })}
        </p>
      )}

      {exportPath && (
        <div
          className="banner"
          role="status"
          style={{ marginBottom: "1rem", justifyContent: "space-between" }}
        >
          <span>
            {t("system.exportSaved", { path: exportPath })}
          </span>
          <Button size="small" variant="secondary" onClick={() => void handleReveal()}>
            {t("system.exportReveal")}
          </Button>
        </div>
      )}
      {exportError && (
        <p className="ui-field-error" role="alert">
          {t("system.exportFailed", { error: exportError })}
        </p>
      )}

      <DataState
        labels={dataStateLabels(t)}
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
                          <td>{providerDisplayName({
                            provider_id: row.provider.provider_id,
                            display_name: row.provider.display_name,
                          })}</td>
                          <td>{row.provider.detected ? t("common.yes") : t("common.no")}</td>
                          <td>{providerSourceLabel(row.provider.source_type, t)}</td>
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
          </div>
        )}
      </DataState>
    </div>
  );
}
