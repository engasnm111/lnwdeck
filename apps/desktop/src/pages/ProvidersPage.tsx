import { useCallback, useEffect, useState } from "react";
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
