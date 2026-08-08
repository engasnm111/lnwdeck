import { useCallback, useEffect, useState } from "react";
import { fetchAnalytics, type AnalyticsRow } from "../../lib/native";
import { DataState, Card, Badge, Button } from "@lnwdeck/ui";
import { dataStateLabels, useI18n } from "../../lib/i18n";
import { formatFullTokenCount } from "../../lib/token-format";

/** Local timestamp as YYYY-MM-DD HH:mm:ss, e.g. 2026-08-07 08:52:12. */
function formatEventTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

export function AnalyticsPage() {
  const { t } = useI18n();
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
          <h2 style={{ fontSize: "1.5rem", fontWeight: 700 }}>{t("nav.analytics")}</h2>
          <p style={{ color: "var(--text-secondary)", fontSize: "0.875rem" }}>
            {t("analytics.subtitle")}
          </p>
        </div>
        <Button variant="secondary" onClick={load} aria-label={t("analytics.refreshAria")}>
          {t("analytics.applyRefresh")}
        </Button>
      </div>

      {/* Dynamic Filter Controls */}
      <Card>
        <div
          role="region"
          aria-label={t("analytics.filters")}
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
              {t("analytics.provider")}
            </label>
            <select
              id="filter-provider"
              className="ui-select"
              value={providerFilter}
              onChange={(e) => setProviderFilter(e.target.value)}
            >
              <option value="">{t("analytics.allProviders")}</option>
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
              {t("models.colModel")}
            </label>
            <select
              id="filter-model"
              className="ui-select"
              value={modelFilter}
              onChange={(e) => setModelFilter(e.target.value)}
            >
              <option value="">{t("analytics.allModels")}</option>
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
              {t("analytics.confidence")}
            </label>
            <select
              id="filter-confidence"
              className="ui-select"
              value={confidenceFilter}
              onChange={(e) => setConfidenceFilter(e.target.value)}
            >
              <option value="">{t("analytics.allConfidence")}</option>
              <option value="High">{t("analytics.high")}</option>
              <option value="Medium">{t("analytics.medium")}</option>
              <option value="Low">{t("analytics.low")}</option>
            </select>
          </div>
        </div>
      </Card>

      <DataState
        labels={dataStateLabels(t)}
        loading={loading}
        error={error}
        isEmpty={rows.length === 0 && !loading}
        emptyFallback={
          <Card title={t("analytics.empty.title")}>
            <p style={{ color: "var(--text-secondary)", marginBottom: "1rem" }}>
              {t("analytics.empty.body")}
            </p>
            <Badge tone="info">{t("analytics.noRecords")}</Badge>
          </Card>
        }
      >
        {/* Summary Card */}
        <div
          role="region"
          aria-label={t("analytics.summary")}
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
              {t("analytics.totalTokens")}
            </span>
            <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
              {formatFullTokenCount(totalTokens)}
            </p>
          </div>
          <div>
            <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
              {t("analytics.totalCost")}
            </span>
            <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
              ${totalCost.toFixed(4)}
            </p>
          </div>
          <div>
            <span style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>
              {t("analytics.recordedEvents")}
            </span>
            <p style={{ fontSize: "1.25rem", fontWeight: 700 }}>
              {rows.length}
            </p>
          </div>
        </div>

        {/* Usage Events Data Table */}
        <Card>
          <table className="ui-table" role="table" aria-label={t("analytics.tableAria")}>
            <thead>
              <tr>
                <th scope="col">{t("analytics.colTimestamp")}</th>
                <th scope="col">{t("system.table.provider")}</th>
                <th scope="col">{t("models.colModel")}</th>
                <th scope="col">{t("analytics.colTokensIn")}</th>
                <th scope="col">{t("analytics.colTokensOut")}</th>
                <th scope="col">{t("analytics.confidence")}</th>
                <th scope="col">{t("costs.colCost")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id}>
                  <td className="ui-table-numeric">{formatEventTimestamp(r.timestamp)}</td>
                  <td>{r.provider_id}</td>
                  <td>{r.model}</td>
                  <td>{formatFullTokenCount(r.tokens_input)}</td>
                  <td>{formatFullTokenCount(r.tokens_output)}</td>
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
