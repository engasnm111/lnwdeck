import { useCallback, useEffect, useState } from "react";
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
import { formatCompact, formatNumber, formatTimestamp } from "../../lib/freshness";

const WINDOWS: Array<{ value: HistoryWindow; label: string }> = [
  { value: "last_24h", label: "24 hours" },
  { value: "last_7d", label: "7 days" },
  { value: "last_30d", label: "30 days" },
  { value: "all", label: "All time" },
];

/**
 * Model usage, aggregated from recorded usage events only.
 *
 * This page shows usage history. It never reads a quota report, so a provider
 * that reports remaining quota without a request history appears here with no
 * rows rather than with invented ones.
 */
export function ModelsPage() {
  const [window, setWindow] = useState<HistoryWindow>("last_7d");
  const [provider, setProvider] = useState<string>("");
  const [data, setData] = useState<UsageHistoryData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await fetchUsageHistory(window, provider || undefined));
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
  }, [window, provider]);

  useEffect(() => {
    void load();
  }, [load]);

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
          <h2 className="page-title">Models</h2>
          <p className="page-subtitle">
            Recorded requests and tokens per model. This is usage history, kept
            separate from provider quota.
          </p>
        </div>
      </div>

      <Toolbar label="Model filters">
        <Tabs
          label="Usage window"
          options={WINDOWS}
          value={window}
          onChange={setWindow}
        />
        <label className="ui-field-label" htmlFor="model-provider">
          Provider
        </label>
        <select
          id="model-provider"
          className="ui-select"
          value={provider}
          onChange={(event) => setProvider(event.target.value)}
        >
          <option value="">All providers</option>
          {(data?.providers ?? []).map((id) => (
            <option key={id} value={id}>
              {id}
            </option>
          ))}
        </select>
      </Toolbar>

      <DataState
        loading={loading}
        error={error}
        isEmpty={data !== null && data.models.length === 0}
        onRetry={() => void load()}
        emptyFallback={
          <Card title="No model usage recorded">
            <p className="ui-inline-note">
              No usage events were recorded in this window. Run a refresh, or
              check the Providers page to see which collectors found a source.
            </p>
          </Card>
        }
      >
        {data && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard
                title="Requests"
                value={formatNumber(data.request_count)}
              />
              <MetricCard
                title="Input tokens"
                value={formatCompact(data.tokens_input)}
              />
              <MetricCard
                title="Output tokens"
                value={formatCompact(data.tokens_output)}
              />
              <MetricCard
                title="Distinct models"
                value={formatNumber(data.models.length)}
              />
            </div>

            {data.daily.length > 0 && (
              <Card
                title="Daily tokens"
                subtitle="Recorded input plus output tokens per day"
              >
                <div className="trend-chart" role="img" aria-label="Daily token totals">
                  {data.daily.map((day) => {
                    const total = day.tokens_input + day.tokens_output;
                    const height = Math.max(2, (total / maxDaily) * 100);
                    return (
                      <div
                        key={day.day}
                        className="trend-bar"
                        style={{ height: `${height}%` }}
                        title={`${day.day}: ${formatCompact(total)} tokens over ${day.request_count} request(s)`}
                      />
                    );
                  })}
                </div>
              </Card>
            )}

            <Card title="Model breakdown">
              <Table
                caption="Recorded usage per model"
                headers={[
                  "Model",
                  "Provider",
                  "Requests",
                  "Input",
                  "Output",
                  "Share",
                  "Last used",
                ]}
              >
                {data.models.map((row) => (
                  <tr key={`${row.provider_id}:${row.model}`}>
                    <td>{row.model}</td>
                    <td>{row.provider_id}</td>
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
                          label={`${row.model} share of tokens`}
                        />
                        <span className="ui-inline-note">
                          {row.token_share_percent === null
                            ? "no tokens recorded"
                            : `${row.token_share_percent.toFixed(1)}%`}
                        </span>
                      </div>
                    </td>
                    <td>{formatTimestamp(row.last_seen_at)}</td>
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
