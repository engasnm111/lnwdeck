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
          <h2 className="page-title">Overview</h2>
          <p className="page-subtitle">
            What lnwdeck recorded on this machine, and what your providers report
            about remaining quota.
          </p>
        </div>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={overview !== null && overview.total_events === 0}
        onRetry={() => void load()}
        emptyFallback={
          <Card title="No usage recorded yet">
            <p className="ui-inline-note">
              No provider artifacts have been ingested. Open the Providers page
              to see which collectors found a source and which are waiting for
              one.
            </p>
          </Card>
        }
      >
        {overview && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard
                title="Recorded events"
                value={formatNumber(overview.total_events)}
                subtitle={`across ${overview.provider_count} provider(s)`}
              />
              <MetricCard
                title="Tokens"
                value={formatCompact(totalTokens)}
                subtitle={`${formatCompact(overview.total_tokens_input)} in / ${formatCompact(overview.total_tokens_output)} out`}
              />
              <MetricCard
                title="Cost"
                value={overview.cost_formatted}
                badge={
                  <Badge tone={costTone(overview.cost_status)}>
                    {overview.cost_status.replace("_", " ")}
                  </Badge>
                }
              />
              <MetricCard
                title="High confidence"
                value={`${Math.round(overview.confidence_coverage * 100)}%`}
                subtitle={`${formatNumber(overview.high_confidence_count)} of ${formatNumber(overview.total_events)} events`}
              />
            </div>

            <div className="channel-split">
              <div className="channel-block">
                <div className="channel-title">
                  <span>Usage history (recorded here)</span>
                  <Badge tone="info">last 7 days</Badge>
                </div>
                {history && history.request_count > 0 ? (
                  <div className="stack-tight">
                    <span className="meta-value">
                      {formatNumber(history.request_count)} request(s),{" "}
                      {formatCompact(history.tokens_input + history.tokens_output)}{" "}
                      tokens
                    </span>
                    <span className="ui-inline-note">
                      Oldest event {formatTimestamp(overview.oldest_event_at)},
                      newest {formatTimestamp(overview.latest_event_at)}
                    </span>
                    <span className="ui-inline-note">
                      {history.models.length} model(s) used
                    </span>
                  </div>
                ) : (
                  <p className="ui-inline-note">
                    Nothing was recorded in the last 7 days.
                  </p>
                )}
              </div>

              <div className="channel-block">
                <div className="channel-title">
                  <span>Quota (reported by providers)</span>
                  {quotaError ? (
                    <Badge tone="danger">unavailable</Badge>
                  ) : (
                    <Badge tone="neutral">
                      {quotaProviders.length} provider(s)
                    </Badge>
                  )}
                </div>
                {quotaError ? (
                  <p className="ui-inline-note">
                    Quota could not be read: {quotaError}
                  </p>
                ) : quotaProviders.length === 0 ? (
                  <p className="ui-inline-note">
                    No provider has reported quota yet.
                  </p>
                ) : (
                  <div className="stack-tight">
                    {lowest ? (
                      <span className="meta-value">
                        Lowest remaining: {lowest.provider} {lowest.label} at{" "}
                        {Math.round(lowest.percent)}%
                      </span>
                    ) : (
                      <span className="meta-value">
                        No provider reports a real limit; quota is shown as usage
                        estimates.
                      </span>
                    )}
                    <span className="ui-inline-note">
                      {withRealLimit.length} of {quotaProviders.length} provider(s)
                      report a limit that can be shown as a percentage
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
