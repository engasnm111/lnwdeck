import { useCallback, useEffect, useState } from "react";
import { fetchOverview, OverviewData } from "../lib/native";
import { DataState } from "@inwdeck/ui";

export function OverviewPage() {
  const [data, setData] = useState<OverviewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await fetchOverview();
      setData(result);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const isEmpty = data !== null && data.total_events === 0;

  return (
    <div>
      <h2>Overview</h2>
      <DataState
        loading={loading}
        error={error}
        isEmpty={isEmpty}
        emptyFallback={<p>No usage data yet. Start tracking to see insights.</p>}
      >
        {data && (
          <div role="region" aria-label="Usage overview">
            <table>
              <tbody>
                <tr>
                  <th>Total Events</th>
                  <td>{data.total_events}</td>
                </tr>
                <tr>
                  <th>Total Tokens In</th>
                  <td>{data.total_tokens_input.toLocaleString()}</td>
                </tr>
                <tr>
                  <th>Total Tokens Out</th>
                  <td>{data.total_tokens_output.toLocaleString()}</td>
                </tr>
                <tr>
                  <th>Providers</th>
                  <td>{data.provider_count}</td>
                </tr>
                <tr>
                  <th>High Confidence</th>
                  <td>{data.high_confidence_count}</td>
                </tr>
                <tr>
                  <th>Confidence Coverage</th>
                  <td>{(data.confidence_coverage * 100).toFixed(1)}%</td>
                </tr>
              </tbody>
            </table>
          </div>
        )}
      </DataState>
    </div>
  );
}
