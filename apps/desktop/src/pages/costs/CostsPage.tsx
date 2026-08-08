import { useCallback, useEffect, useState } from "react";
import {
  Badge,
  Card,
  DataState,
  MetricCard,
  Table,
  Tabs,
  Toolbar,
} from "@lnwdeck/ui";
import { fetchCosts, type CostBreakdownData, type HistoryWindow } from "../../lib/native";
import { formatCompact, formatNumber } from "../../lib/freshness";
import { useI18n } from "../../lib/i18n";

const WINDOWS: Array<{ value: HistoryWindow; labelKey: string }> = [
  { value: "last_24h", labelKey: "costs.window24h" },
  { value: "last_7d", labelKey: "costs.window7d" },
  { value: "last_30d", labelKey: "costs.window30d" },
  { value: "all", labelKey: "costs.windowAll" },
];

/**
 * Cost breakdown per provider and model.
 *
 * Rows without a pricing entry are listed with their token totals and marked as
 * unpriced. They are never charged at another model rate and never counted as
 * zero in the total.
 */
export function CostsPage() {
  const { t } = useI18n();
  const [window, setWindow] = useState<HistoryWindow>("last_30d");
  const [data, setData] = useState<CostBreakdownData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await fetchCosts(window));
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
  }, [window]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("nav.costs")}</h2>
          <p className="page-subtitle">{t("costs.subtitle")}</p>
        </div>
      </div>

      <Toolbar label={t("costs.windowLabel")}>
        <Tabs
          label={t("costs.windowLabel")}
          options={WINDOWS.map((window) => ({ value: window.value, label: t(window.labelKey) }))}
          value={window}
          onChange={setWindow}
        />
      </Toolbar>

      <DataState
        loading={loading}
        error={error}
        isEmpty={data !== null && data.rows.length === 0}
        onRetry={() => void load()}
        emptyFallback={
          <Card title={t("costs.empty.title")}>
            <p className="ui-inline-note">
              {t("costs.empty.body")}
            </p>
          </Card>
        }
      >
        {data && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard
                title={t("costs.pricedTotal")}
                value={data.priced_total}
                subtitle={t("costs.pricedModels", { count: String(data.priced_rows) })}
              />
              <MetricCard
                title={t("costs.estimatedModels")}
                value={formatNumber(data.estimated_rows)}
                subtitle={t("costs.estimatedRows", { count: formatNumber(data.estimated_rows) })}
                badge={
                  data.estimated_rows > 0 ? (
                    <Badge tone="warning">{t("costs.estimated")}</Badge>
                  ) : (
                    <Badge tone="success">{t("costs.fullCoverage")}</Badge>
                  )
                }
              />
              <MetricCard
                title={t("costs.modelsInWindow")}
                value={formatNumber(data.rows.length)}
              />
            </div>

            <Card title={t("costs.byModel")}>
              <Table
                caption={t("costs.tableCaption")}
                headers={[
                  t("costs.colProvider"),
                  t("costs.colModel"),
                  t("costs.colRequests"),
                  t("costs.colInput"),
                  t("costs.colOutput"),
                  t("costs.colCost"),
                  t("costs.colPricing"),
                ]}
              >
                {data.rows.map((row) => (
                  <tr key={`${row.provider_id}:${row.model}`}>
                    <td>{row.provider_id}</td>
                    <td>{row.model}</td>
                    <td className="ui-table-numeric">
                      {formatNumber(row.request_count)}
                    </td>
                    <td className="ui-table-numeric">
                      {formatCompact(row.tokens_input)}
                    </td>
                    <td className="ui-table-numeric">
                      {formatCompact(row.tokens_output)}
                    </td>
                    <td className="ui-table-numeric">
                      {row.cost || t("costs.notPriced")}
                    </td>
                    <td>
                      <Badge
                        tone={
                          row.pricing_status === "priced"
                            ? "success"
                            : row.pricing_status === "estimated"
                              ? "warning"
                              : "danger"
                        }
                      >
                        {row.pricing_status === "estimated"
                          ? t("costs.estimated")
                          : row.pricing_status}
                      </Badge>
                    </td>
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
