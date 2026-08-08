import { useCallback, useEffect, useMemo, useState } from "react";
import { Badge, Button, Card, DataState, MetricCard } from "@lnwdeck/ui";
import {
  fetchQuotaDashboard,
  fetchUsageDashboard,
  type DashboardRange,
  type QuotaDashboardData,
  type UsageDashboardData,
} from "../lib/native";
import { formatNumber, formatTimestamp } from "../lib/freshness";
import { TokenValue } from "../components/TokenValue";
import { dataStateLabels, useI18n } from "../lib/i18n";

const RANGE_OPTIONS: DashboardRange[] = [
  "day",
  "week",
  "month",
  "year",
  "total",
  "custom",
];

function providerIcon(providerId: string): string {
  return providerId.slice(0, 2).toUpperCase() || "AI";
}

function dateLabel(value: string, language: string): string {
  return formatTimestamp(`${value}T12:00:00Z`, language);
}

function percent(value: number, total: number): string {
  if (total <= 0) return "0%";
  return `${Math.round((value / total) * 100)}%`;
}

/** TokenTracker-style usage dashboard backed by one consistent query model. */
export function OverviewPage() {
  const { t, language } = useI18n();
  const [range, setRange] = useState<DashboardRange>("month");
  const [customStart, setCustomStart] = useState("");
  const [customEnd, setCustomEnd] = useState("");
  const [providerId, setProviderId] = useState("");
  const [dashboard, setDashboard] = useState<UsageDashboardData | null>(null);
  const [available, setAvailable] = useState<UsageDashboardData | null>(null);
  const [quota, setQuota] = useState<QuotaDashboardData | null>(null);
  const [quotaError, setQuotaError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const query = useMemo(
    () => ({
      range,
      ...(range === "custom" && customStart ? { start: customStart } : {}),
      ...(range === "custom" && customEnd ? { end: customEnd } : {}),
      ...(providerId ? { provider_id: providerId } : {}),
    }),
    [customEnd, customStart, providerId, range],
  );

  const baseQuery = useMemo(
    () => ({
      range,
      ...(range === "custom" && customStart ? { start: customStart } : {}),
      ...(range === "custom" && customEnd ? { end: customEnd } : {}),
    }),
    [customEnd, customStart, range],
  );

  const load = useCallback(async () => {
    if (range === "custom" && (!customStart || !customEnd)) return;
    setLoading(true);
    setError(null);
    try {
      const [allResult, selectedResult] = await Promise.all([
        fetchUsageDashboard(baseQuery),
        providerId ? fetchUsageDashboard(query) : null,
      ]);
      setAvailable(allResult);
      setDashboard(selectedResult ?? allResult);
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
    try {
      setQuota(await fetchQuotaDashboard());
      setQuotaError(null);
    } catch (loadError) {
      setQuotaError(
        loadError instanceof Error ? loadError.message : String(loadError),
      );
    }
  }, [baseQuery, customEnd, customStart, providerId, query, range]);

  useEffect(() => {
    void load();
  }, [load]);

  const maxTrend = Math.max(...(dashboard?.trend.map((point) => point.total_tokens) ?? [0]), 1);
  const maxHeat = Math.max(...(dashboard?.heatmap.map((cell) => cell.total_tokens) ?? [0]), 1);
  const providerOptions = available?.providers ?? [];
  const quotaProviders = quota?.providers ?? [];
  const withRealLimit = quotaProviders.filter((provider) =>
    provider.windows.some((window) => window.remaining_percent !== null),
  );
  const lowest = withRealLimit
    .flatMap((provider) =>
      provider.windows
        .filter((window) => window.remaining_percent !== null)
        .map((window) => ({
          provider: provider.display_name,
          label: window.label,
          percent: window.remaining_percent as number,
        })),
    )
    .sort((a, b) => a.percent - b.percent)[0];

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("overview.title")}</h2>
          <p className="page-subtitle">{t("dashboard.subtitle")}</p>
        </div>
        <div className="dashboard-range" role="tablist" aria-label={t("dashboard.rangeAria")}>
          {RANGE_OPTIONS.map((option) => (
            <Button
              key={option}
              size="small"
              variant={range === option ? "primary" : "ghost"}
              role="tab"
              aria-selected={range === option}
              onClick={() => setRange(option)}
            >
              {t(`dashboard.range.${option}`)}
            </Button>
          ))}
        </div>
      </div>

      {range === "custom" && (
        <div className="dashboard-custom-range" role="group" aria-label={t("dashboard.customAria")}>
          <label>
            <span>{t("dashboard.start")}</span>
            <input
              type="date"
              value={customStart}
              onChange={(event) => setCustomStart(event.target.value)}
            />
          </label>
          <span className="dashboard-range-separator">{t("dashboard.rangeTo")}</span>
          <label>
            <span>{t("dashboard.end")}</span>
            <input
              type="date"
              value={customEnd}
              min={customStart || undefined}
              onChange={(event) => setCustomEnd(event.target.value)}
            />
          </label>
        </div>
      )}

      <DataState
        labels={dataStateLabels(t)}
        loading={loading}
        error={error}
        isEmpty={dashboard !== null && dashboard.request_count === 0}
        onRetry={() => void load()}
        emptyFallback={
          <Card title={t("dashboard.emptyTitle")}>
            <p className="ui-inline-note">{t("dashboard.emptyBody")}</p>
          </Card>
        }
      >
        {dashboard && (
          <div className="stack dashboard-page">
            <div className="grid-metrics dashboard-metrics">
              <MetricCard
                title={t("dashboard.totalTokens")}
                value={
                  <TokenValue
                    value={dashboard.total_tokens}
                    label={t("dashboard.totalTokens")}
                    exactLabel={t("dashboard.showFull")}
                  />
                }
                subtitle={t("dashboard.duration", { days: formatNumber(dashboard.duration_days) })}
              />
              <MetricCard
                title={t("dashboard.inputTokens")}
                value={
                  <TokenValue
                    value={dashboard.tokens_input}
                    label={t("dashboard.inputTokens")}
                    exactLabel={t("dashboard.showFull")}
                  />
                }
                subtitle={t("dashboard.requests", { count: formatNumber(dashboard.request_count) })}
              />
              <MetricCard
                title={t("dashboard.outputTokens")}
                value={
                  <TokenValue
                    value={dashboard.tokens_output}
                    label={t("dashboard.outputTokens")}
                    exactLabel={t("dashboard.showFull")}
                  />
                }
                subtitle={t("dashboard.sessions", { count: formatNumber(dashboard.session_count) })}
              />
              <MetricCard
                title={t("dashboard.providers")}
                value={formatNumber(dashboard.provider_count)}
                subtitle={t("dashboard.filteredBy", {
                  provider: providerId ? providerId : t("dashboard.all"),
                })}
              />
            </div>

            <Card
              title={t("dashboard.providerBreakdown")}
              subtitle={t("dashboard.providerBreakdownHint")}
            >
              <div className="dashboard-provider-filters" role="toolbar" aria-label={t("dashboard.providerFilterAria")}>
                <button
                  type="button"
                  className={`dashboard-provider-filter ${providerId === "" ? "is-active" : ""}`}
                  aria-pressed={providerId === ""}
                  onClick={() => setProviderId("")}
                >
                  <span className="dashboard-provider-icon dashboard-provider-icon-all">ALL</span>
                  <span>
                    <strong>{t("dashboard.all")}</strong>
                    <small>{percent(dashboard.total_tokens, available?.total_tokens ?? dashboard.total_tokens)}</small>
                  </span>
                </button>
                {providerOptions.map((provider) => (
                  <button
                    type="button"
                    className={`dashboard-provider-filter ${providerId === provider.provider_id ? "is-active" : ""}`}
                    aria-pressed={providerId === provider.provider_id}
                    key={provider.provider_id}
                    onClick={() => setProviderId(provider.provider_id)}
                  >
                    <span className="dashboard-provider-icon">{providerIcon(provider.provider_id)}</span>
                    <span>
                      <strong>{provider.provider_id}</strong>
                      <small>{percent(provider.total_tokens, available?.total_tokens ?? dashboard.total_tokens)}</small>
                    </span>
                  </button>
                ))}
              </div>
            </Card>

            <div className="dashboard-visual-grid">
              <Card title={t("dashboard.usageTrend")} subtitle={t("dashboard.usageTrendHint")}>
                {dashboard.trend.length > 0 ? (
                  <div className="dashboard-trend" role="img" aria-label={t("dashboard.usageTrendAria")}>
                    {dashboard.trend.map((point) => (
                      <div className="dashboard-trend-column" key={point.bucket} title={`${dateLabel(point.bucket, language)}: ${formatNumber(point.total_tokens)}`}>
                        <div
                          className="dashboard-trend-bar"
                          style={{ height: `${Math.max((point.total_tokens / maxTrend) * 100, 4)}%` }}
                        />
                        <span>{point.bucket.slice(5)}</span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="ui-inline-note">{t("dashboard.noTrend")}</p>
                )}
              </Card>
              <Card title={t("dashboard.activityHeatmap")} subtitle={t("dashboard.activityHeatmapHint")}>
                {dashboard.heatmap.length > 0 ? (
                  <div className="dashboard-heatmap" role="img" aria-label={t("dashboard.activityHeatmapAria")}>
                    {dashboard.heatmap.map((cell) => (
                      <span
                        key={cell.day}
                        className="dashboard-heatmap-cell"
                        style={{ opacity: 0.22 + (cell.total_tokens / maxHeat) * 0.78 }}
                        title={`${dateLabel(cell.day, language)}: ${formatNumber(cell.total_tokens)}`}
                      />
                    ))}
                  </div>
                ) : (
                  <p className="ui-inline-note">{t("dashboard.noHeatmap")}</p>
                )}
              </Card>
            </div>

            <Card
              title={t("dashboard.sessionsTitle")}
              subtitle={t("dashboard.sessionsHint", { count: formatNumber(dashboard.session_count) })}
            >
              {dashboard.sessions.length === 0 ? (
                <p className="ui-inline-note">{t("dashboard.noSessions")}</p>
              ) : (
                <div className="ui-table-wrap">
                  <table className="ui-table" aria-label={t("dashboard.sessionsAria")}>
                    <thead>
                      <tr>
                        <th>{t("dashboard.session")}</th>
                        <th>{t("dashboard.provider")}</th>
                        <th className="ui-table-numeric">{t("dashboard.tokens")}</th>
                        <th className="ui-table-numeric">{t("dashboard.requestsLabel")}</th>
                        <th>{t("dashboard.lastActivity")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {dashboard.sessions.map((session) => (
                        <tr key={session.session_hash}>
                          <td>
                            <strong>{session.display_name}</strong>
                            <small className="dashboard-session-id">{session.session_hash}</small>
                          </td>
                          <td>
                            <div className="dashboard-session-providers">
                              {session.providers.map((provider) => (
                                <span className="dashboard-provider-pill" key={provider.provider_id}>
                                  <span className="dashboard-provider-icon dashboard-provider-icon-small">{providerIcon(provider.provider_id)}</span>
                                  {provider.provider_id}
                                </span>
                              ))}
                            </div>
                          </td>
                          <td className="ui-table-numeric">
                            <TokenValue
                              value={session.total_tokens}
                              label={`${session.display_name} ${t("dashboard.tokens")}`}
                              exactLabel={t("dashboard.showFull")}
                            />
                          </td>
                          <td className="ui-table-numeric">{formatNumber(session.request_count)}</td>
                          <td>{formatTimestamp(session.last_seen_at, language)}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </Card>

            <div className="channel-split">
              <div className="channel-block">
                <div className="channel-title">
                  <span>{t("overview.quotaChannel")}</span>
                  {quotaError ? (
                    <Badge tone="danger">{t("overview.unavailable")}</Badge>
                  ) : (
                    <Badge tone="neutral">{t("overview.providerCount", { count: String(quotaProviders.length) })}</Badge>
                  )}
                </div>
                {quotaError ? (
                  <p className="ui-inline-note">{t("overview.quotaReadFailed", { error: quotaError })}</p>
                ) : quotaProviders.length === 0 ? (
                  <p className="ui-inline-note">{t("overview.noQuotaYet")}</p>
                ) : (
                  <div className="stack-tight">
                    {lowest ? (
                      <span className="meta-value">{t("overview.lowestRemaining", { provider: lowest.provider, label: lowest.label, percent: String(Math.round(lowest.percent)) })}</span>
                    ) : (
                      <span className="meta-value">{t("overview.noRealLimit")}</span>
                    )}
                    <span className="ui-inline-note">{t("overview.limitReported", { count: String(withRealLimit.length), total: String(quotaProviders.length) })}</span>
                  </div>
                )}
              </div>
              <div className="channel-block">
                <div className="channel-title"><span>{t("dashboard.rangeLabel")}</span><Badge tone="info">{t(`dashboard.range.${range}`)}</Badge></div>
                <span className="ui-inline-note">{t("dashboard.utcHint")}</span>
                {dashboard.start && dashboard.end && (
                  <span className="ui-inline-note">{formatTimestamp(dashboard.start, language)} {t("dashboard.rangeTo")} {formatTimestamp(dashboard.end, language)}</span>
                )}
              </div>
            </div>
          </div>
        )}
      </DataState>
    </div>
  );
}
