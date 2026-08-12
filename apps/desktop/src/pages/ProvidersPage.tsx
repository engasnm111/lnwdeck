import { useCallback, useState } from "react";
import { Badge, Button, Card, DataState, ProgressBar } from "@lnwdeck/ui";
import {
  fetchProviders,
  fetchQuotaDashboard,
  refreshProvider,
  type DetailedProviderInfo,
  type ProviderQuotaCard,
  type QuotaDashboardData,
} from "../lib/native";
import { usePageLoad } from "../lib/use-page-load";
import { formatCompact, formatTimestamp } from "../lib/freshness";
import { ProviderLogo, providerDisplayName } from "../components/ProviderLogo";
import { dataStateLabels, useI18n } from "../lib/i18n";
import {
  providerAuthLabel,
  providerCostLabel,
  providerHealthLabel,
  providerKindLabel,
  providerQuotaSummaryLabel,
  providerSourceLabel,
  providerSupportLabel,
} from "../lib/providerText";

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
interface ProvidersPageData {
  providers: DetailedProviderInfo[];
  quota: QuotaDashboardData | null;
}

export function ProvidersPage() {
  const { t, language } = useI18n();
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const {
    data,
    loading,
    error,
    reload,
  } = usePageLoad<ProvidersPageData>({
    load: async () => {
      const providers = await fetchProviders();
      let quota: QuotaDashboardData | null = null;
      try {
        quota = await fetchQuotaDashboard();
      } catch {
        // The quota channel is independent; the provider table still renders.
      }
      return { providers, quota };
    },
    deps: [],
    refreshEvents: ["quota-updated"],
  });

  const providers = data?.providers ?? [];
  const quota = data?.quota ?? null;

  const handleRefreshProvider = useCallback(
    async (providerId: string) => {
      setRefreshingId(providerId);
      setActionError(null);
      try {
        await refreshProvider(providerId);
        await reload();
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
    [reload],
  );

  const quotaFor = (providerId: string): ProviderQuotaCard[] =>
    quota?.providers.filter((card) => card.provider_id === providerId) ?? [];

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("nav.providers")}</h2>
          <p className="page-subtitle">{t("providers.subtitle")}</p>
        </div>
      </div>

      {actionError && (
        <p className="ui-field-error" role="alert">
          {actionError}
        </p>
      )}

      <DataState
        labels={dataStateLabels(t)}
        loading={loading}
        error={error}
        isEmpty={providers.length === 0}
        onRetry={() => void reload()}
      >
        <div className="grid-cards">
          {providers.map((provider) => {
            const cards = quotaFor(provider.provider_id);
            const displayName = providerDisplayName(provider);
            return (
              <Card
                key={provider.provider_id}
                title={displayName}
                subtitle={`${provider.vendor} - ${providerSourceLabel(provider.source_type, t)}`}
                action={
                  <div className="row">
                    <Badge tone={healthTone(provider.health_status)}>
                      {providerHealthLabel(provider.health_status, t)}
                    </Badge>
                    <Button
                      size="small"
                      onClick={() =>
                        void handleRefreshProvider(provider.provider_id)
                      }
                      disabled={refreshingId === provider.provider_id}
                      aria-label={t("system.refreshProvider", { provider: displayName })}
                    >
                      {refreshingId === provider.provider_id
                        ? t("topbar.refreshing")
                        : t("system.refresh")}
                    </Button>
                  </div>
                }
              >
                <div className="provider-card-heading">
                  <ProviderLogo providerId={provider.provider_id} displayName={displayName} vendor={provider.vendor} />
                </div>
                <div className="row">
                  <Badge tone={supportTone(provider.usage_support)}>
                    {t("providers.historyLabel", { support: providerSupportLabel(provider.usage_support, t) })}
                  </Badge>
                  <Badge tone={supportTone(provider.quota_support)}>
                    {t("providers.quotaLabel", { support: providerSupportLabel(provider.quota_support, t) })}
                  </Badge>
                  <Badge tone="neutral">{t("providers.authLabel", { requirement: providerAuthLabel(provider.auth_requirement, t) })}</Badge>
                </div>

                <div className="channel-split">
                  <div className="channel-block">
                    <div className="channel-title">
                      <span>{t("providers.historyChannel")}</span>
                    </div>
                    <div className="provider-card-meta">
                      <div className="meta-item">
                        <span className="meta-label">{t("providers.events")}</span>
                        <span className="meta-value">
                          {provider.event_count.toLocaleString()}
                        </span>
                      </div>
                      <div className="meta-item">
                        <span className="meta-label">{t("overview.tokens")}</span>
                        <span className="meta-value">
                          {formatCompact(provider.total_tokens)}
                        </span>
                      </div>
                      <div className="meta-item">
                        <span className="meta-label">{t("providers.lastActivity")}</span>
                        <span className="meta-value">
                          {formatTimestamp(provider.last_sync, language)}
                        </span>
                      </div>
                      <div className="meta-item">
                        <span className="meta-label">{t("providers.pricing")}</span>
                        <span className="meta-value">
                          {providerCostLabel(provider.cost_support, t)}
                        </span>
                      </div>
                    </div>
                  </div>

                  <div className="channel-block">
                    <div className="channel-title">
                      <span>{t("providers.quotaChannel")}</span>
                      {cards.length === 1 && (
                        <Badge tone="neutral">{providerSourceLabel(cards[0]?.source ?? "", t)}</Badge>
                      )}
                    </div>
                    {cards.length === 0 ? (
                      <p className="ui-inline-note">
                        {providerQuotaSummaryLabel(provider.quota_summary, t)}
                      </p>
                    ) : cards.every((card) => card.windows.length === 0) ? (
                      cards.some((card) => card.error_code === "SOURCE_REQUIRES_IDE") ? (
                        <p className="ui-inline-note">
                          {t("providers.quota.requiresIde")}
                        </p>
                      ) : (
                        <p className="ui-inline-note">
                          {providerQuotaSummaryLabel(provider.quota_summary, t)}
                        </p>
                      )
                    ) : (
                      <div className="stack-tight">
                        {cards.map((card) => {
                          const accountLabel = card.account_index == null
                            ? null
                            : t("providers.account", { number: String(card.account_index) });
                          const cardDisplayName = accountLabel
                            ? `${displayName} - ${accountLabel}`
                            : displayName;
                          return (
                            <div
                              key={`${card.provider_id}-${card.account_index ?? "default"}`}
                              className="stack-tight"
                            >
                              {accountLabel && (
                                <div className="bar-row-head">
                                  <span>{accountLabel}</span>
                                  <span className="ui-mono">{providerSourceLabel(card.source, t)}</span>
                                </div>
                              )}
                              {card.windows.length === 0 ? (
                                <p className="ui-inline-note">
                                  {providerQuotaSummaryLabel(provider.quota_summary, t)}
                                </p>
                              ) : card.windows.map((window) => (
                                <div
                                  key={`${card.account_index ?? "default"}-${window.window_key}`}
                                  className="bar-row"
                                >
                                  <div className="bar-row-head">
                                    <span>{window.label}</span>
                                    <span className="ui-mono">
                                      {window.remaining_percent === null
                                        ? t("providers.kindUsed", { used: formatCompact(window.used), kind: providerKindLabel(window.kind, t) })
                                        : t("providers.percentLeft", { percent: String(Math.round(window.remaining_percent)) })}
                                    </span>
                                  </div>
                                  <ProgressBar
                                    percent={window.remaining_percent}
                                    label={t("providers.remainingLabel", { provider: cardDisplayName, label: window.label })}
                                  />
                                </div>
                              ))}
                              <span className="ui-inline-note">
                                {t("providers.collectedAt", { time: formatTimestamp(card.collected_at, language) })}
                                {card.plan ? t("providers.planSuffix", { plan: card.plan }) : ""}
                              </span>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                </div>

                {provider.last_error_code && (
                  <p className="ui-inline-note">
                    {t("providers.lastError", { error: provider.last_error_code })}
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
