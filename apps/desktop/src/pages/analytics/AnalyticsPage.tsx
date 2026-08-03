import { DataState } from "@inwdeck/ui";
import { useCallback, useEffect, useMemo, useState } from "react";

type Confidence = "Low" | "Medium" | "High";

interface UsageRow {
  id: string;
  timestamp: string;
  provider_id: string;
  model: string;
  tokens_input: number;
  tokens_output: number;
  confidence: Confidence;
  cost: string;
}

interface Filters {
  provider: string;
  model: string;
  confidence: string;
}

function useAnalytics() {
  const [rows, setRows] = useState<UsageRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setRows([]);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  return { rows, loading, error, reload: load };
}

export function AnalyticsPage() {
  const { rows, loading, error } = useAnalytics();
  const [filters, setFilters] = useState<Filters>({
    provider: "",
    model: "",
    confidence: "",
  });

  const filtered = useMemo(() => {
    return rows.filter((r) => {
      if (filters.provider && r.provider_id !== filters.provider) return false;
      if (filters.model && r.model !== filters.model) return false;
      if (filters.confidence && r.confidence !== filters.confidence) return false;
      return true;
    });
  }, [rows, filters]);

  const totalTokens = filtered.reduce((s, r) => s + r.tokens_input + r.tokens_output, 0);
  const totalCost = filtered.reduce((s, r) => s + parseFloat(r.cost || "0"), 0);

  return (
    <div>
      <h2>Analytics</h2>
      <div role="region" aria-label="Filters">
        <label htmlFor="filter-provider">Provider</label>
        <select
          id="filter-provider"
          value={filters.provider}
          onChange={(e) => setFilters((f) => ({ ...f, provider: e.target.value }))}
        >
          <option value="">All</option>
          <option value="openai">OpenAI</option>
          <option value="anthropic">Anthropic</option>
        </select>

        <label htmlFor="filter-model">Model</label>
        <select
          id="filter-model"
          value={filters.model}
          onChange={(e) => setFilters((f) => ({ ...f, model: e.target.value }))}
        >
          <option value="">All</option>
          <option value="gpt-4o">GPT-4o</option>
          <option value="claude-3">Claude 3</option>
        </select>

        <label htmlFor="filter-confidence">Confidence</label>
        <select
          id="filter-confidence"
          value={filters.confidence}
          onChange={(e) => setFilters((f) => ({ ...f, confidence: e.target.value }))}
        >
          <option value="">All</option>
          <option value="High">High</option>
          <option value="Medium">Medium</option>
          <option value="Low">Low</option>
        </select>
      </div>

      <DataState
        loading={loading}
        error={error}
        isEmpty={rows.length === 0 && !loading}
        emptyFallback={<p>No usage data yet. Connect a provider to start tracking.</p>}
      >
        <div role="region" aria-label="Summary" style={{ margin: "1rem 0" }}>
          <p>Total Tokens: <strong>{totalTokens.toLocaleString()}</strong></p>
          <p>Total Cost: <strong>${totalCost.toFixed(4)}</strong></p>
        </div>

        <table role="table" aria-label="Usage events">
          <thead>
            <tr>
              <th>Timestamp</th>
              <th>Provider</th>
              <th>Model</th>
              <th>Tokens In</th>
              <th>Tokens Out</th>
              <th>Confidence</th>
              <th>Cost</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((r) => (
              <tr key={r.id}>
                <td>{r.timestamp}</td>
                <td>{r.provider_id}</td>
                <td>{r.model}</td>
                <td>{r.tokens_input}</td>
                <td>{r.tokens_output}</td>
                <td>{r.confidence}</td>
                <td>{r.cost}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </DataState>
    </div>
  );
}
