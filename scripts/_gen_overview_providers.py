"""One-off generator for the Overview and Providers pages, plus small fixes."""

import re

FILES = {}

FILES["apps/desktop/src/pages/OverviewPage.tsx"] = '''import { useCallback, useEffect, useState } from "react";
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
'''

FILES["apps/desktop/src/pages/ProvidersPage.tsx"] = '''import { useCallback, useEffect, useState } from "react";
import { Badge, Button, Card, DataState, ProgressBar } from "@lnwdeck/ui";
import {
  fetchProviders,
  fetchQuotaDashboard,
  refreshProvider,
  type DetailedProviderInfo,
  type ProviderQuotaCard,
  type QuotaDashboardData,
} from "../lib/native";
import { formatCompact, formatTimestamp } from "../lib/freshness";

function healthTone(status: string) {
  if (status.startsWith("Error")) {
    return "danger" as const;
  }
  if (status === "Healthy") {
    return "success" as const;
  }
  if (status === "Not supported") {
    return "neutral" as const;
  }
  return "warning" as const;
}

function supportTone(support: string) {
  if (support === "supported") {
    return "success" as const;
  }
  if (support === "local estimate") {
    return "info" as const;
  }
  return "neutral" as const;
}

/**
 * Providers.
 *
 * Each card states what the adapter declares it can collect, what the last
 * detection and collection actually found, and the quota the provider reported.
 * A provider that collects nothing is labelled "Not supported" rather than
 * healthy.
 */
export function ProvidersPage() {
  const [providers, setProviders] = useState<DetailedProviderInfo[]>([]);
  const [quota, setQuota] = useState<QuotaDashboardData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setProviders(await fetchProviders());
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
    try {
      setQuota(await fetchQuotaDashboard());
    } catch {
      // The quota channel is independent; the provider table still renders.
      setQuota(null);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleRefreshProvider = useCallback(
    async (providerId: string) => {
      setRefreshingId(providerId);
      setActionError(null);
      try {
        await refreshProvider(providerId);
        await load();
      } catch (refreshError) {
        setActionError(
          refreshError instanceof Error
            ? refreshError.message
            : String(refreshError),
        );
      } finally {
        setRefreshingId(null);
      }
    },
    [load],
  );

  const quotaFor = (providerId: string): ProviderQuotaCard | undefined =>
    quota?.providers.find((card) => card.provider_id === providerId);

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">Providers</h2>
          <p className="page-subtitle">
            Ten built-in adapters. Each one declares which channels it supports;
            the runtime never records a successful collection for a channel that
            is not implemented.
          </p>
        </div>
      </div>

      {actionError && (
        <p className="ui-field-error" role="alert">
          {actionError}
        </p>
      )}

      <DataState
        loading={loading}
        error={error}
        isEmpty={providers.length === 0}
        onRetry={() => void load()}
      >
        <div className="grid-cards">
          {providers.map((provider) => {
            const card = quotaFor(provider.provider_id);
            return (
              <Card
                key={provider.provider_id}
                title={provider.display_name}
                subtitle={`${provider.vendor} - ${provider.source_type}`}
                action={
                  <div className="row">
                    <Badge tone={healthTone(provider.health_status)}>
                      {provider.health_status}
                    </Badge>
                    <Button
                      size="small"
                      onClick={() =>
                        void handleRefreshProvider(provider.provider_id)
                      }
                      disabled={refreshingId === provider.provider_id}
                      aria-label={`Refresh ${provider.display_name}`}
                    >
                      {refreshingId === provider.provider_id
                        ? "Refreshing"
                        : "Refresh"}
                    </Button>
                  </div>
                }
              >
                <div className="row">
                  <Badge tone={supportTone(provider.usage_support)}>
                    history: {provider.usage_support}
                  </Badge>
                  <Badge tone={supportTone(provider.quota_support)}>
                    quota: {provider.quota_support}
                  </Badge>
                  <Badge tone="neutral">auth: {provider.auth_requirement}</Badge>
                </div>

                <div className="channel-split">
                  <div className="channel-block">
                    <div className="channel-title">
                      <span>Usage history (recorded)</span>
                    </div>
                    <div className="provider-card-meta">
                      <div className="meta-item">
                        <span className="meta-label">Events</span>
                        <span className="meta-value">
                          {provider.event_count.toLocaleString()}
                        </span>
                      </div>
                      <div className="meta-item">
                        <span className="meta-label">Tokens</span>
                        <span className="meta-value">
                          {formatCompact(provider.total_tokens)}
                        </span>
                      </div>
                      <div className="meta-item">
                        <span className="meta-label">Last activity</span>
                        <span className="meta-value">
                          {formatTimestamp(provider.last_sync)}
                        </span>
                      </div>
                      <div className="meta-item">
                        <span className="meta-label">Pricing</span>
                        <span className="meta-value">
                          {provider.cost_support}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="channel-block">
                    <div className="channel-title">
                      <span>Quota (reported)</span>
                      {card && <Badge tone="neutral">{card.source}</Badge>}
                    </div>
                    {!card ? (
                      <p className="ui-inline-note">
                        {provider.quota_summary}
                      </p>
                    ) : card.windows.length === 0 ? (
                      <p className="ui-inline-note">
                        {provider.quota_summary}
                      </p>
                    ) : (
                      <div className="stack-tight">
                        {card.windows.map((window) => (
                          <div key={window.window_key} className="bar-row">
                            <div className="bar-row-head">
                              <span>{window.label}</span>
                              <span className="ui-mono">
                                {window.remaining_percent === null
                                  ? `${formatCompact(window.used)} ${window.kind} used`
                                  : `${Math.round(window.remaining_percent)}% left`}
                              </span>
                            </div>
                            <ProgressBar
                              percent={window.remaining_percent}
                              label={`${provider.display_name} ${window.label} remaining`}
                            />
                          </div>
                        ))}
                        <span className="ui-inline-note">
                          Collected {formatTimestamp(card.collected_at)}
                          {card.plan ? ` - plan ${card.plan}` : ""}
                        </span>
                      </div>
                    )}
                  </div>
                </div>

                {provider.last_error_code && (
                  <p className="ui-inline-note">
                    Last collector error: {provider.last_error_code}
                  </p>
                )}
              </Card>
            );
          })}
        </div>
      </DataState>
    </div>
  );
}
'''


def patch(path: str, replacements: list[tuple[str, str]]) -> None:
    with open(path, encoding="utf-8") as handle:
        content = handle.read()
    for old, new in replacements:
        if old not in content:
            raise SystemExit(f"{path}: pattern not found: {old[:60]}")
        content = content.replace(old, new)
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(content)
    print("patched", path)


def main() -> None:
    for path, content in FILES.items():
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(content)
        print("wrote", path)

    # The UI kit components no longer reference the React namespace directly.
    for component in ["ProgressBar", "Tabs", "Toggle"]:
        path = f"packages/ui/src/{component}.tsx"
        with open(path, encoding="utf-8") as handle:
            content = handle.read()
        content = re.sub(r'^import React from "react";\n\n', "", content)
        with open(path, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(content)
        print("cleaned import in", path)

    # Card no longer takes a raw style prop; the page uses a class instead.
    patch(
        "apps/desktop/src/pages/analytics/AnalyticsPage.tsx",
        [('style={{ marginBottom: "1rem" }}', "")],
    )


if __name__ == "__main__":
    main()
