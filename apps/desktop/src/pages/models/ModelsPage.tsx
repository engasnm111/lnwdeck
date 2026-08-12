import { useState } from "react";
import {
  Card,
  DataState,
  MetricCard,
  ProgressBar,
  Table,
  Tabs,
  Toolbar,
} from "@lnwdeck/ui";
import {
  fetchUsageHistory,
  type HistoryWindow,
  type UsageHistoryData,
} from "../../lib/native";
import { usePageLoad } from "../../lib/use-page-load";
import { formatCompact, formatNumber, formatTimestamp } from "../../lib/freshness";
import { dataStateLabels, useI18n } from "../../lib/i18n";
import { modelDisplayName, providerDisplayName } from "../../components/ProviderLogo";

const WINDOWS: Array<{ value: HistoryWindow; labelKey: string }> = [
  { value: "last_24h", labelKey: "costs.window24h" },
  { value: "last_7d", labelKey: "costs.window7d" },
  { value: "last_30d", labelKey: "costs.window30d" },
  { value: "all", labelKey: "costs.windowAll" },
];

/**
 * Model usage, aggregated from recorded usage events only.
 *
 * This page shows usage history. It never reads a quota report, so a provider
 * that reports remaining quota without a request history appears here with no
 * rows rather than with invented ones.
 */
export function ModelsPage() {
  const { t, language } = useI18n();
  const [window, setWindow] = useState<HistoryWindow>("last_7d");
  const [provider, setProvider] = useState<string>("");
  const {
    data,
    loading,
    error,
    reload,
  } = usePageLoad<UsageHistoryData>({
    load: () => fetchUsageHistory(window, provider || undefined),
    deps: [window, provider],
    refreshEvents: ["usage-updated"],
  });

  const maxDaily = data
    ? Math.max(
        1,
        ...data.daily.map((day) => day.tokens_input + day.tokens_output),
      )
    : 1;

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("nav.models")}</h2>
          <p className="page-subtitle">{t("models.subtitle")}</p>
        </div>
      </div>

      <Toolbar label={t("models.filters")}>
        <Tabs
          label={t("models.windowLabel")}
          options={WINDOWS.map((window) => ({ value: window.value, label: t(window.labelKey) }))}
          value={window}
          onChange={setWindow}
        />
        <label className="ui-field-label" htmlFor="model-provider">
          {t("models.providerLabel")}
        </label>
        <select
          id="model-provider"
          className="ui-select"
          value={provider}
          onChange={(event) => setProvider(event.target.value)}
        >
          <option value="">{t("models.allProviders")}</option>
          {(data?.providers ?? []).map((id) => (
            <option key={id} value={id}>
              {providerDisplayName({ provider_id: id, display_name: id })}
            </option>
          ))}
        </select>
      </Toolbar>

      <DataState
        labels={dataStateLabels(t)}
        loading={loading}
        error={error}
        isEmpty={data !== null && data.models.length === 0}
        onRetry={() => void reload()}
        emptyFallback={
          <Card title={t("models.empty.title")}>
            <p className="ui-inline-note">
              {t("models.empty.body")}
            </p>
          </Card>
        }
      >
        {data && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard
                title={t("models.requests")}
                value={formatNumber(data.request_count)}
              />
              <MetricCard
                title={t("models.inputTokens")}
                value={formatCompact(data.tokens_input)}
              />
              <MetricCard
                title={t("models.outputTokens")}
                value={formatCompact(data.tokens_output)}
              />
              <MetricCard
                title={t("models.distinctModels")}
                value={formatNumber(data.models.length)}
              />
            </div>

            {data.daily.length > 0 && (
              <Card
                title={t("models.dailyTokens")}
                subtitle={t("models.dailySubtitle")}
              >
                <div className="trend-chart" role="img" aria-label={t("models.dailyAria")}>
                  {data.daily.map((day) => {
                    const total = day.tokens_input + day.tokens_output;
                    const height = Math.max(2, (total / maxDaily) * 100);
                    return (
                      <div
                        key={day.day}
                        className="trend-bar"
                        style={{ height: `${height}%` }}
                        title={t("models.dailyTitle", { day: day.day, tokens: formatCompact(total), count: String(day.request_count) })}
                      />
                    );
                  })}
                </div>
              </Card>
            )}

            <Card title={t("models.breakdown")}>
              <Table
                caption={t("models.tableCaption")}
                headers={[
                  t("models.colModel"),
                  t("models.colProvider"),
                  t("models.colRequests"),
                  t("models.colInput"),
                  t("models.colOutput"),
                  t("models.colShare"),
                  t("models.colLastUsed"),
                ]}
              >
                {data.models.map((row) => (
                  <tr key={`${row.provider_id}:${row.model}`}>
                    <td>{modelDisplayName(row.model, t("analytics.unknownModel"))}</td>
                    <td>{providerDisplayName({ provider_id: row.provider_id, display_name: row.provider_id })}</td>
                    <td className="ui-table-numeric">
                      {formatNumber(row.request_count)}
                    </td>
                    <td className="ui-table-numeric">
                      {formatCompact(row.tokens_input)}
                    </td>
                    <td className="ui-table-numeric">
                      {formatCompact(row.tokens_output)}
                    </td>
                    <td>
                      <div className="stack-tight">
                        <ProgressBar
                          percent={row.token_share_percent}
                          label={t("models.shareLabel", { model: row.model })}
                        />
                        <span className="ui-inline-note">
                          {row.token_share_percent === null
                            ? t("models.noTokens")
                            : `${row.token_share_percent.toFixed(1)}%`}
                        </span>
                      </div>
                    </td>
                    <td>{formatTimestamp(row.last_seen_at, language)}</td>
                  </tr>
                ))}
              </Table>
            </Card>
          </div>
        )}
      </DataState>
    </div>
  );
}
