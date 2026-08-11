import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Badge, Button, Card, DataState, MetricCard } from "@lnwdeck/ui";
import {
  fetchQuotaDashboard,
  fetchUsageDashboard,
  type DashboardRange,
  type DashboardHeatmapCell,
  type DashboardProviderUsage,
  type QuotaDashboardData,
  type UsageDashboardData,
} from "../lib/native";
import { formatNumber, formatTimestamp } from "../lib/freshness";
import { TokenValue } from "../components/TokenValue";
import { ProviderLogo, providerDisplayName } from "../components/ProviderLogo";
import { dataStateLabels, useI18n } from "../lib/i18n";

const RANGE_OPTIONS: DashboardRange[] = [
  "day",
  "week",
  "month",
  "year",
  "total",
  "custom",
];

function dateLabel(value: string, language: string): string {
  return formatTimestamp(`${value}T12:00:00Z`, language);
}

function percent(value: number, total: number): string {
  if (total <= 0) return "0%";
  return `${Math.round((value / total) * 100)}%`;
}

function providerLabel(provider: DashboardProviderUsage): string {
  return providerDisplayName(provider);
}

function DailyTokenValue({
  value,
  label,
  exactLabel,
}: {
  value: number;
  label: string;
  exactLabel: string;
}) {
  if (value === 0) {
    return <span aria-label={`${label}: 0`}>—</span>;
  }
  return <TokenValue value={value} label={label} exactLabel={exactLabel} />;
}

interface HeatmapWeek {
  key: string;
  monthLabel: string;
  cells: Array<DashboardHeatmapCell | null>;
}

function parseCalendarDay(value: string): Date {
  const [year, month, day] = value.split("-").map(Number);
  return new Date(year, month - 1, day, 12, 0, 0, 0);
}

function calendarDayKey(value: Date): string {
  return [value.getFullYear(), value.getMonth() + 1, value.getDate()]
    .map((part, index) => (index === 0 ? String(part).padStart(4, "0") : String(part).padStart(2, "0")))
    .join("-");
}

function DatePickerIcon() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <rect x="2.5" y="3.5" width="11" height="10" rx="2" />
      <path d="M2.5 6.5h11M5 2v3M11 2v3" />
    </svg>
  );
}

/** Builds the Sunday-to-Saturday columns used by the TokenTracker heatmap. */
function buildHeatmapWeeks(
  cells: DashboardHeatmapCell[],
  language: string,
): HeatmapWeek[] {
  if (cells.length === 0) return [];
  const byDay = new Map(cells.map((cell) => [cell.day, cell]));
  const dates = cells.map((cell) => parseCalendarDay(cell.day));
  const first = new Date(Math.min(...dates.map((date) => date.getTime())));
  const last = new Date(Math.max(...dates.map((date) => date.getTime())));
  const start = new Date(first);
  start.setDate(start.getDate() - start.getDay());
  const end = new Date(last);
  end.setDate(end.getDate() + (6 - end.getDay()));
  const monthFormatter = new Intl.DateTimeFormat(language, { month: "short" });
  const weeks: HeatmapWeek[] = [];

  for (const cursor = new Date(start); cursor <= end; cursor.setDate(cursor.getDate() + 7)) {
    const weekStart = new Date(cursor);
    const weekCells = Array.from({ length: 7 }, (_, offset) => {
      const day = new Date(weekStart);
      day.setDate(day.getDate() + offset);
      return byDay.get(calendarDayKey(day)) ?? null;
    });
    let monthLabel = weeks.length === 0 ? monthFormatter.format(weekStart) : "";
    for (let offset = 0; offset < 7; offset += 1) {
      const day = new Date(weekStart);
      day.setDate(day.getDate() + offset);
      if (day.getDate() === 1) {
        monthLabel = monthFormatter.format(day);
        break;
      }
    }
    weeks.push({ key: calendarDayKey(weekStart), monthLabel, cells: weekCells });
  }
  return weeks;
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
  const requestVersion = useRef(0);
  const customStartRef = useRef<HTMLInputElement | null>(null);
  const customEndRef = useRef<HTMLInputElement | null>(null);

  const openDatePicker = useCallback((input: HTMLInputElement | null) => {
    if (!input) return;
    input.focus();
    const pickerInput = input as HTMLInputElement & {
      showPicker?: () => void;
    };
    try {
      pickerInput.showPicker?.();
    } catch {
      // Some WebView versions do not expose the native picker outside a trusted click.
      // The focused type=date input remains usable as the fallback control.
    }
  }, []);

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
    const version = ++requestVersion.current;
    const isCurrent = () => requestVersion.current === version;
    setLoading(true);
    setError(null);
    try {
      const [allResult, selectedResult] = await Promise.all([
        fetchUsageDashboard(baseQuery),
        providerId ? fetchUsageDashboard(query) : null,
      ]);
      if (isCurrent()) {
        setAvailable(allResult);
        setDashboard(selectedResult ?? allResult);
      }
    } catch (loadError) {
      if (isCurrent()) {
        setError(
          loadError instanceof Error ? loadError : new Error(String(loadError)),
        );
      }
    } finally {
      if (isCurrent()) setLoading(false);
    }
    try {
      const nextQuota = await fetchQuotaDashboard();
      if (isCurrent()) {
        setQuota(nextQuota);
        setQuotaError(null);
      }
    } catch (loadError) {
      if (isCurrent()) {
        setQuotaError(
          loadError instanceof Error ? loadError.message : String(loadError),
        );
      }
    }
  }, [baseQuery, customEnd, customStart, providerId, query, range]);

  useEffect(() => {
    void load();
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    void listen("usage-updated", () => {
      void load();
    })
      .then((cleanup) => {
        if (cancelled) {
          cleanup();
        } else {
          unlisten = cleanup;
        }
      })
      .catch(() => {
        // The web test shell has no native event bus; polling/manual retry still works.
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [load]);

  const maxTrend = Math.max(...(dashboard?.trend.map((point) => point.total_tokens) ?? [0]), 1);
  const maxHeat = Math.max(...(dashboard?.heatmap.map((cell) => cell.total_tokens) ?? [0]), 1);
  const heatmapWeeks = buildHeatmapWeeks(dashboard?.heatmap ?? [], language);
  const today = calendarDayKey(new Date());
  const dailyBreakdownRows = (dashboard?.trend ?? [])
    .filter((point) => point.bucket <= today)
    .sort((a, b) => b.bucket.localeCompare(a.bucket));
  const providerOptions = available?.providers ?? [];
  const selectedProvider = providerOptions.find((provider) => provider.provider_id === providerId);
  const selectedProviderLabel = selectedProvider
    ? providerLabel(selectedProvider)
    : providerId
      ? providerDisplayName({ providerId, displayName: providerId })
      : t("dashboard.all");
  const quotaProviders = quota?.providers ?? [];
  // Only usable readings count toward the headline number: a provider whose
  // last collection failed (or whose source needs the Antigravity IDE open)
  // keeps its stored windows out of the derived "lowest remaining" line.
  const withRealLimit = quotaProviders.filter(
    (provider) =>
      (provider.status === "fresh" || provider.status === "stale") &&
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
            <span className="dashboard-date-control">
              <input
                ref={customStartRef}
                className="dashboard-date-input"
                type="date"
                lang={language}
                aria-label={t("dashboard.start")}
                value={customStart}
                onChange={(event) => setCustomStart(event.target.value)}
              />
              <button
                type="button"
                className="dashboard-date-picker-trigger"
                data-date-picker-trigger="true"
                aria-label={`${t("dashboard.start")}: ${t("dashboard.pickDate")}`}
                title={`${t("dashboard.start")}: ${t("dashboard.pickDate")}`}
                onClick={() => openDatePicker(customStartRef.current)}
              >
                <DatePickerIcon />
              </button>
            </span>
          </label>
          <span className="dashboard-range-separator">{t("dashboard.rangeTo")}</span>
          <label>
            <span>{t("dashboard.end")}</span>
            <span className="dashboard-date-control">
              <input
                ref={customEndRef}
                className="dashboard-date-input"
                type="date"
                lang={language}
                aria-label={t("dashboard.end")}
                value={customEnd}
                min={customStart || undefined}
                onChange={(event) => setCustomEnd(event.target.value)}
              />
              <button
                type="button"
                className="dashboard-date-picker-trigger"
                data-date-picker-trigger="true"
                aria-label={`${t("dashboard.end")}: ${t("dashboard.pickDate")}`}
                title={`${t("dashboard.end")}: ${t("dashboard.pickDate")}`}
                onClick={() => openDatePicker(customEndRef.current)}
              >
                <DatePickerIcon />
              </button>
            </span>
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
                  provider: selectedProviderLabel,
                })}
              />
            </div>

            <div className="dashboard-layout">
              <div className="dashboard-secondary-column">
                <Card
                  title={t("dashboard.activityHeatmap")}
                  subtitle={t("dashboard.activityHeatmapHint")}
                  className="dashboard-heatmap-card dashboard-heatmap-card-fixed"
                >
                  {heatmapWeeks.length > 0 ? (
                    <div
                      className="dashboard-heatmap"
                      role="img"
                      aria-label={t("dashboard.activityHeatmapAria")}
                    >
                      <div
                        className="dashboard-heatmap-months"
                        style={{ gridTemplateColumns: `28px repeat(${heatmapWeeks.length}, 14px)` }}
                        aria-hidden="true"
                      >
                        <span />
                        {heatmapWeeks.map((week) => <span key={week.key}>{week.monthLabel}</span>)}
                      </div>
                      <div className="dashboard-heatmap-grid-shell">
                        <div className="dashboard-heatmap-weekdays" aria-hidden="true">
                          {[
                            t("dashboard.weekday.sun"),
                            t("dashboard.weekday.mon"),
                            t("dashboard.weekday.tue"),
                            t("dashboard.weekday.wed"),
                            t("dashboard.weekday.thu"),
                            t("dashboard.weekday.fri"),
                            t("dashboard.weekday.sat"),
                          ].map((weekday) => <span key={weekday}>{weekday}</span>)}
                        </div>
                        <div
                          className="dashboard-heatmap-weeks"
                          style={{ gridTemplateColumns: `repeat(${heatmapWeeks.length}, 14px)` }}
                        >
                          {heatmapWeeks.map((week) => (
                            <div className="dashboard-heatmap-week" key={week.key}>
                              {week.cells.map((cell, index) => cell ? (
                                <span
                                  key={cell.day}
                                  className="dashboard-heatmap-cell"
                                  data-day={cell.day}
                                  style={{ opacity: 0.22 + (cell.total_tokens / maxHeat) * 0.78 }}
                                  title={`${dateLabel(cell.day, language)}: ${formatNumber(cell.total_tokens)}`}
                                />
                              ) : (
                                <span className="dashboard-heatmap-cell dashboard-heatmap-cell-empty" key={`${week.key}-${index}`} aria-hidden="true" />
                              ))}
                            </div>
                          ))}
                        </div>
                      </div>
                    </div>
                  ) : (
                    <p className="ui-inline-note">{t("dashboard.noHeatmap")}</p>
                  )}
                </Card>

                <Card title={t("dashboard.usageTrend")} subtitle={t("dashboard.usageTrendHint")}>
                  {dashboard.trend.length > 0 ? (
                    <div className="dashboard-trend" role="img" aria-label={t("dashboard.usageTrendAria")}>
                      {dashboard.trend.map((point) => (
                        <div className="dashboard-trend-column" key={point.bucket} title={`${dateLabel(point.bucket, language)}: ${formatNumber(point.total_tokens)}`}>
                          <div
                            className="dashboard-trend-bar"
                            style={{ height: `${Math.max((point.total_tokens / maxTrend) * 100, point.total_tokens > 0 ? 4 : 1)}%` }}
                          />
                          <span
                            className="dashboard-trend-label"
                            data-trend-label={point.bucket.slice(5)}
                          >
                            {point.bucket.slice(5)}
                          </span>
                        </div>
                      ))}
                    </div>
                  ) : (
                    <p className="ui-inline-note">{t("dashboard.noTrend")}</p>
                  )}
                </Card>
              </div>

              <div className="dashboard-primary-column">
                <Card
                  title={t("dashboard.providerBreakdown")}
                  subtitle={t("dashboard.providerBreakdownHint")}
                  className="dashboard-provider-breakdown"
                >
                  <div className="dashboard-provider-filters" role="toolbar" aria-label={t("dashboard.providerFilterAria")}>
                    <button
                      type="button"
                      className={`dashboard-provider-filter ${providerId === "" ? "is-active" : ""}`}
                      aria-pressed={providerId === ""}
                      onClick={() => setProviderId("")}
                    >
                      <ProviderLogo providerId="all" displayName={t("dashboard.all")} />
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
                        <ProviderLogo providerId={provider.provider_id} displayName={providerLabel(provider)} vendor={provider.vendor} />
                        <span>
                          <strong>{providerLabel(provider)}</strong>
                          <small>{percent(provider.total_tokens, available?.total_tokens ?? dashboard.total_tokens)}</small>
                        </span>
                      </button>
                    ))}
                  </div>
                </Card>

                <Card
                  title={t("dashboard.dailyBreakdownTitle")}
                  subtitle={t("dashboard.dailyBreakdownHint")}
                  className="dashboard-daily-breakdown-card dashboard-daily-breakdown-card-fixed"
                >
                  {dailyBreakdownRows.length === 0 ? (
                    <p className="ui-inline-note">{t("dashboard.noDailyBreakdown")}</p>
                  ) : (
                    <div
                      className="ui-table-wrap dashboard-daily-breakdown-wrap dashboard-daily-breakdown-wrap-fixed"
                      role="region"
                      aria-label={t("dashboard.dailyBreakdownAria")}
                      tabIndex={0}
                    >
                      <table className="ui-table dashboard-daily-breakdown-table" aria-label={t("dashboard.dailyBreakdownAria")}>
                        <thead className="ui-table-head-themed">
                          <tr>
                            <th>{t("dashboard.date")}</th>
                            <th className="ui-table-numeric">{t("dashboard.totalTokens")}</th>
                            <th className="ui-table-numeric">{t("dashboard.inputTokens")}</th>
                            <th className="ui-table-numeric">{t("dashboard.outputTokens")}</th>
                            <th className="ui-table-numeric">{t("dashboard.cachedTokens")}</th>
                            <th className="ui-table-numeric">{t("dashboard.reasoningTokens")}</th>
                            <th className="ui-table-numeric">{t("dashboard.requestsLabel")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {dailyBreakdownRows.map((point) => {
                            const cachedTokens = point.tokens_cached + point.tokens_cache_write;
                            return (
                              <tr key={point.bucket}>
                                <td>
                                  <time dateTime={point.bucket} title={dateLabel(point.bucket, language)}>
                                    {point.bucket}
                                  </time>
                                </td>
                                <td className="ui-table-numeric">
                                  <DailyTokenValue
                                    value={point.total_tokens}
                                    label={`${point.bucket} ${t("dashboard.totalTokens")}`}
                                    exactLabel={t("dashboard.showFull")}
                                  />
                                </td>
                                <td className="ui-table-numeric">
                                  <DailyTokenValue
                                    value={point.tokens_input}
                                    label={`${point.bucket} ${t("dashboard.inputTokens")}`}
                                    exactLabel={t("dashboard.showFull")}
                                  />
                                </td>
                                <td className="ui-table-numeric">
                                  <DailyTokenValue
                                    value={point.tokens_output}
                                    label={`${point.bucket} ${t("dashboard.outputTokens")}`}
                                    exactLabel={t("dashboard.showFull")}
                                  />
                                </td>
                                <td className="ui-table-numeric">
                                  <DailyTokenValue
                                    value={cachedTokens}
                                    label={`${point.bucket} ${t("dashboard.cachedTokens")}`}
                                    exactLabel={t("dashboard.showFull")}
                                  />
                                </td>
                                <td className="ui-table-numeric">
                                  <DailyTokenValue
                                    value={point.tokens_reasoning}
                                    label={`${point.bucket} ${t("dashboard.reasoningTokens")}`}
                                    exactLabel={t("dashboard.showFull")}
                                  />
                                </td>
                                <td className="ui-table-numeric">
                                  {point.request_count === 0 ? "—" : formatNumber(point.request_count)}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  )}
                </Card>
              </div>
            </div>

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
