import { useCallback, useEffect, useState } from "react";
import {
  Badge,
  Button,
  Card,
  DataState,
  Field,
  ProgressBar,
  Toggle,
} from "@lnwdeck/ui";
import {
  deleteBudget,
  fetchBudgets,
  fetchProviders,
  saveBudget,
  type BudgetOverviewData,
  type BudgetPeriod,
  type BudgetProgressData,
  type DetailedProviderInfo,
} from "../../lib/native";
import { formatCompact, formatTimestamp } from "../../lib/freshness";

const PERIODS: BudgetPeriod[] = ["daily", "weekly", "monthly"];

function stateTone(state: BudgetProgressData["state"]) {
  switch (state) {
    case "exceeded":
      return "danger" as const;
    case "warning":
      return "warning" as const;
    case "under":
      return "success" as const;
    default:
      return "neutral" as const;
  }
}

function barTone(state: BudgetProgressData["state"]) {
  switch (state) {
    case "exceeded":
      return "danger" as const;
    case "warning":
      return "warning" as const;
    default:
      return "success" as const;
  }
}

function scopeLabel(
  progress: BudgetProgressData,
  providers: DetailedProviderInfo[],
): string {
  if (progress.budget.scope.kind === "global") {
    return "All providers";
  }
  const id = progress.budget.scope.provider_id ?? "";
  return providers.find((p) => p.provider_id === id)?.display_name ?? id;
}

/**
 * Budgets configured by the user, with progress measured against recorded
 * usage. Nothing is preconfigured: with no budgets the page says so instead of
 * showing a reassuring status.
 */
export function BudgetsPage() {
  const [data, setData] = useState<BudgetOverviewData | null>(null);
  const [providers, setProviders] = useState<DetailedProviderInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const [scope, setScope] = useState<"global" | "provider">("global");
  const [providerId, setProviderId] = useState("");
  const [period, setPeriod] = useState<BudgetPeriod>("monthly");
  const [costLimit, setCostLimit] = useState("");
  const [tokenLimit, setTokenLimit] = useState("");
  const [warnPercent, setWarnPercent] = useState("80");
  const [enabled, setEnabled] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [overview, providerList] = await Promise.all([
        fetchBudgets(),
        fetchProviders(),
      ]);
      setData(overview);
      setProviders(providerList);
      if (!providerId && providerList.length > 0) {
        setProviderId(providerList[0].provider_id);
      }
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
  }, [providerId]);

  useEffect(() => {
    void load();
    // The provider default is set once; re-running on every id change would
    // fight the user selection.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleSave = useCallback(async () => {
    setSaving(true);
    setFormError(null);
    try {
      await saveBudget({
        scope,
        provider_id: scope === "provider" ? providerId : undefined,
        period,
        cost_limit: costLimit.trim(),
        token_limit: tokenLimit.trim() ? Number(tokenLimit) : undefined,
        warn_percent: Number(warnPercent),
        enabled,
      });
      setCostLimit("");
      setTokenLimit("");
      await load();
    } catch (saveError) {
      setFormError(
        saveError instanceof Error ? saveError.message : String(saveError),
      );
    } finally {
      setSaving(false);
    }
  }, [scope, providerId, period, costLimit, tokenLimit, warnPercent, enabled, load]);

  const handleDelete = useCallback(
    async (id: number) => {
      setFormError(null);
      try {
        await deleteBudget(id);
        await load();
      } catch (deleteError) {
        setFormError(
          deleteError instanceof Error
            ? deleteError.message
            : String(deleteError),
        );
      }
    },
    [load],
  );

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">Budgets</h2>
          <p className="page-subtitle">
            Spending and token caps you configure. Progress is measured against
            recorded usage for the period; usage that cannot be priced is
            reported separately rather than counted as zero.
          </p>
        </div>
      </div>

      <div className="stack">
        <Card title="Add or update a budget">
          <div className="form-grid">
            <Field label="Scope" htmlFor="budget-scope">
              <select
                id="budget-scope"
                className="ui-select"
                value={scope}
                onChange={(event) =>
                  setScope(event.target.value as "global" | "provider")
                }
              >
                <option value="global">All providers</option>
                <option value="provider">One provider</option>
              </select>
            </Field>
            {scope === "provider" && (
              <Field label="Provider" htmlFor="budget-provider">
                <select
                  id="budget-provider"
                  className="ui-select"
                  value={providerId}
                  onChange={(event) => setProviderId(event.target.value)}
                >
                  {providers.map((provider) => (
                    <option
                      key={provider.provider_id}
                      value={provider.provider_id}
                    >
                      {provider.display_name}
                    </option>
                  ))}
                </select>
              </Field>
            )}
            <Field label="Period" htmlFor="budget-period">
              <select
                id="budget-period"
                className="ui-select"
                value={period}
                onChange={(event) =>
                  setPeriod(event.target.value as BudgetPeriod)
                }
              >
                {PERIODS.map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </Field>
            <Field
              label="Cost limit"
              htmlFor="budget-cost"
              hint="Decimal amount, for example 25.00"
            >
              <input
                id="budget-cost"
                className="ui-input"
                inputMode="decimal"
                value={costLimit}
                onChange={(event) => setCostLimit(event.target.value)}
              />
            </Field>
            <Field
              label="Token limit"
              htmlFor="budget-tokens"
              hint="Optional; leave empty for none"
            >
              <input
                id="budget-tokens"
                className="ui-input"
                inputMode="numeric"
                value={tokenLimit}
                onChange={(event) => setTokenLimit(event.target.value)}
              />
            </Field>
            <Field label="Warn at" htmlFor="budget-warn" hint="Percent of limit">
              <input
                id="budget-warn"
                className="ui-input"
                inputMode="numeric"
                value={warnPercent}
                onChange={(event) => setWarnPercent(event.target.value)}
              />
            </Field>
            <Toggle
              id="budget-enabled"
              label="Enabled"
              checked={enabled}
              onChange={setEnabled}
            />
            <Button
              variant="primary"
              onClick={() => void handleSave()}
              disabled={saving}
            >
              {saving ? "Saving" : "Save budget"}
            </Button>
          </div>
          {formError && (
            <p className="ui-field-error" role="alert">
              {formError}
            </p>
          )}
        </Card>

        <DataState
          loading={loading}
          error={error}
          isEmpty={data !== null && data.budgets.length === 0}
          onRetry={() => void load()}
          emptyFallback={
            <Card title="No budgets configured">
              <p className="ui-inline-note">
                No spending or token caps have been set, so nothing is being
                tracked against a limit.
              </p>
            </Card>
          }
        >
          {data && (
            <div className="grid-cards">
              {data.budgets.map((progress) => (
                <Card
                  key={progress.budget.id}
                  title={`${scopeLabel(progress, providers)} - ${progress.budget.period}`}
                  subtitle={`Period started ${formatTimestamp(progress.period_start)}`}
                  action={
                    <div className="row">
                      <Badge tone={stateTone(progress.state)}>
                        {progress.state}
                      </Badge>
                      <Button
                        variant="danger"
                        size="small"
                        onClick={() => void handleDelete(progress.budget.id)}
                        aria-label={`Delete budget ${progress.budget.id}`}
                      >
                        Delete
                      </Button>
                    </div>
                  }
                >
                  <div className="stack-tight">
                    <div className="bar-row">
                      <div className="bar-row-head">
                        <span>Cost</span>
                        <span className="ui-mono">
                          {progress.cost_used}
                          {progress.budget.cost_limit
                            ? ` / ${progress.budget.cost_limit}`
                            : " (no cost limit)"}
                        </span>
                      </div>
                      <ProgressBar
                        percent={progress.cost_percent}
                        tone={barTone(progress.state)}
                        label="Cost budget used"
                      />
                    </div>
                    <div className="bar-row">
                      <div className="bar-row-head">
                        <span>Tokens</span>
                        <span className="ui-mono">
                          {formatCompact(progress.tokens_used)}
                          {progress.budget.token_limit
                            ? ` / ${formatCompact(progress.budget.token_limit)}`
                            : " (no token limit)"}
                        </span>
                      </div>
                      <ProgressBar
                        percent={progress.token_percent}
                        tone={barTone(progress.state)}
                        label="Token budget used"
                      />
                    </div>
                    <span className="ui-inline-note">
                      {progress.request_count} request(s) in this period
                      {progress.unpriced_tokens > 0
                        ? `; ${formatCompact(progress.unpriced_tokens)} tokens could not be priced`
                        : ""}
                    </span>
                    {!progress.budget.enabled && (
                      <Badge tone="neutral">Disabled</Badge>
                    )}
                  </div>
                </Card>
              ))}
            </div>
          )}
        </DataState>
      </div>
    </div>
  );
}
