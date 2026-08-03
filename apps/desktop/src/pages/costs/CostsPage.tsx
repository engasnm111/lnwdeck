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

const WINDOWS: Array<{ value: HistoryWindow; label: string }> = [
  { value: "last_24h", label: "24 hours" },
  { value: "last_7d", label: "7 days" },
  { value: "last_30d", label: "30 days" },
  { value: "all", label: "All time" },
];

/**
 * Cost breakdown per provider and model.
 *
 * Rows without a pricing entry are listed with their token totals and marked as
 * unpriced. They are never charged at another model rate and never counted as
 * zero in the total.
 */
export function CostsPage() {
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
          <h2 className="page-title">Costs</h2>
          <p className="page-subtitle">
            Calculated from recorded usage and the local pricing catalog. A model
            without a catalog entry is listed as unpriced rather than estimated
            at another rate.
          </p>
        </div>
      </div>

      <Toolbar label="Cost window">
        <Tabs
          label="Cost window"
          options={WINDOWS}
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
          <Card title="No costs recorded">
            <p className="ui-inline-note">
              No usage has been recorded in this window, so there is nothing to
              price yet.
            </p>
          </Card>
        }
      >
        {data && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard
                title="Priced total"
                value={data.priced_total}
                subtitle={`${data.priced_rows} priced model(s)`}
              />
              <MetricCard
                title="Unpriced models"
                value={formatNumber(data.unpriced_rows)}
                subtitle={`${formatCompact(data.unpriced_tokens)} tokens without pricing`}
                badge={
                  data.unpriced_rows > 0 ? (
                    <Badge tone="warning">Incomplete pricing</Badge>
                  ) : (
                    <Badge tone="success">Full coverage</Badge>
                  )
                }
              />
              <MetricCard
                title="Models in window"
                value={formatNumber(data.rows.length)}
              />
            </div>

            <Card title="Cost by model">
              <Table
                caption="Recorded usage and calculated cost per provider and model"
                headers={[
                  "Provider",
                  "Model",
                  "Requests",
                  "Input",
                  "Output",
                  "Cost",
                  "Pricing",
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
                      {row.cost ?? "not priced"}
                    </td>
                    <td>
                      <Badge tone={row.cost ? "success" : "warning"}>
                        {row.pricing_status}
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
