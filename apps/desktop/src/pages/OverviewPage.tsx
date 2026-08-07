import { useCallback, useEffect, useState } from "react";
import { Badge, Card, DataState, MetricCard } from "@lnwdeck/ui";
import {
  fetchOverview,
  fetchQuotaDashboard,
  fetchUsageHistory,
  type OverviewData,
  type QuotaDashboardData,
  type UsageHistoryData,
} from "../lib/native";
import {
  formatCompact,
  formatNumber,
  formatTimestamp,
} from "../lib/freshness";
import { useI18n } from "../lib/i18n";

function costTone(status: string) {
  switch (status) {
    case "exact":
      return "success" as const;
    case "estimated":
      return "info" as const;
    case "missing_pricing":
      return "warning" as const;
    default:
      return "neutral" as const;
  }
}

/**
 * Overview.
 *
 * Usage history and provider quota are shown as two separate blocks, because
 * they come from two independent channels: history is what lnwdeck recorded,
 * quota is what a provider reported. Neither is derived from the other.
 */
export function OverviewPage() {
  const { t, language } = useI18n();
  const [overview, setOverview] = useState<OverviewData | null>(null);
  const [history, setHistory] = useState<UsageHistoryData | null>(null);
  const [quota, setQuota] = useState<QuotaDashboardData | null>(null);
  const [quotaError, setQuotaError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    setQuotaError(null);
    try {
      const [overviewResult, historyResult] = await Promise.all([
        fetchOverview(),
        fetchUsageHistory("last_7d"),
      ]);
      setOverview(overviewResult);
      setHistory(historyResult);
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
    // The quota channel is independent: its failure must not blank the usage
    // figures, and vice versa.
    try {
      setQuota(await fetchQuotaDashboard());
    } catch (loadError) {
      setQuotaError(
        loadError instanceof Error ? loadError.message : String(loadError),
      );
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const totalTokens = overview
    ? overview.total_tokens_input + overview.total_tokens_output
    : 0;

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
          <p className="page-subtitle">{t("overview.subtitle")}</p>
        </div>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={overview !== null && overview.total_events === 0}
        onRetry={() => void load()}
        emptyFallback={
          <Card title={t("overview.empty.title")}>
            <p className="ui-inline-note">{t("overview.empty.body")}</p>
          </Card>
        }
      >
        {overview && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard
                title={t("overview.recordedEvents")}
                value={formatNumber(overview.total_events)}
                subtitle={t("overview.acrossProviders", { count: String(overview.provider_count) })}
              />
              <MetricCard
                title={t("overview.tokens")}
                value={formatCompact(totalTokens)}
                subtitle={t("overview.tokensInOut", { input: formatCompact(overview.total_tokens_input), output: formatCompact(overview.total_tokens_output) })}
              />
              <MetricCard
                title={t("overview.cost")}
                value={overview.cost_formatted}
                badge={
                  <Badge tone={costTone(overview.cost_status)}>
                    {overview.cost_status.replace("_", " ")}
                  </Badge>
                }
              />
              <MetricCard
                title={t("overview.highConfidence")}
                value={`${Math.round(overview.confidence_coverage * 100)}%`}
                subtitle={t("overview.confidenceEvents", { count: formatNumber(overview.high_confidence_count), total: formatNumber(overview.total_events) })}
              />
            </div>

            <div className="channel-split">
              <div className="channel-block">
                <div className="channel-title">
                  <span>{t("overview.historyChannel")}</span>
                  <Badge tone="info">{t("overview.last7Days")}</Badge>
                </div>
                {history && history.request_count > 0 ? (
                  <div className="stack-tight">
                    <span className="meta-value">
                      {t("overview.requestCount", { count: formatNumber(history.request_count), tokens: formatCompact(history.tokens_input + history.tokens_output) })}
                    </span>
                    <span className="ui-inline-note">
                      {t("overview.oldestNewest", { oldest: formatTimestamp(overview.oldest_event_at, language), newest: formatTimestamp(overview.latest_event_at, language) })}
                    </span>
                    <span className="ui-inline-note">
                      {t("overview.modelsUsed", { count: String(history.models.length) })}
                    </span>
                  </div>
                ) : (
                  <p className="ui-inline-note">
                    {t("overview.nothingRecorded")}
                  </p>
                )}
              </div>

              <div className="channel-block">
                <div className="channel-title">
                  <span>{t("overview.quotaChannel")}</span>
                  {quotaError ? (
                    <Badge tone="danger">{t("overview.unavailable")}</Badge>
                  ) : (
                    <Badge tone="neutral">
                      {t("overview.providerCount", { count: String(quotaProviders.length) })}
                    </Badge>
                  )}
                </div>
                {quotaError ? (
                  <p className="ui-inline-note">
                    {t("overview.quotaReadFailed", { error: quotaError })}
                  </p>
                ) : quotaProviders.length === 0 ? (
                  <p className="ui-inline-note">
                    {t("overview.noQuotaYet")}
                  </p>
                ) : (
                  <div className="stack-tight">
                    {lowest ? (
                      <span className="meta-value">
                        {t("overview.lowestRemaining", { provider: lowest.provider, label: lowest.label, percent: String(Math.round(lowest.percent)) })}
                      </span>
                    ) : (
                      <span className="meta-value">
                        {t("overview.noRealLimit")}
                      </span>
                    )}
                    <span className="ui-inline-note">
                      {t("overview.limitReported", { count: String(withRealLimit.length), total: String(quotaProviders.length) })}
                    </span>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}
      </DataState>
    </div>
  );
}
