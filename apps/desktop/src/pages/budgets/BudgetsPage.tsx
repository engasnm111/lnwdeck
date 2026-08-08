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
import { dataStateLabels, useI18n } from "../../lib/i18n";

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
  t: (key: string) => string,
): string {
  if (progress.budget.scope.kind === "global") {
    return t("budgets.allProviders");
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
  const { t, language } = useI18n();
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
          <h2 className="page-title">{t("nav.budgets")}</h2>
          <p className="page-subtitle">{t("budgets.subtitle")}</p>
        </div>
      </div>

      <div className="stack">
        <Card title={t("budgets.addTitle")}>
          <div className="budget-form">
            <div className="budget-form-fields">
              <Field label={t("budgets.scope")} htmlFor="budget-scope">
                <select
                  id="budget-scope"
                  className="ui-select"
                  value={scope}
                  onChange={(event) =>
                    setScope(event.target.value as "global" | "provider")
                  }
                >
                  <option value="global">{t("budgets.allProviders")}</option>
                  <option value="provider">{t("budgets.oneProvider")}</option>
                </select>
              </Field>
              {scope === "provider" && (
                <Field label={t("models.providerLabel")} htmlFor="budget-provider">
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
              <Field label={t("budgets.period")} htmlFor="budget-period">
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
                label={t("budgets.costLimit")}
                htmlFor="budget-cost"
                hint={t("budgets.costLimitHint")}
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
                label={t("budgets.tokenLimit")}
                htmlFor="budget-tokens"
                hint={t("budgets.tokenLimitHint")}
              >
                <input
                  id="budget-tokens"
                  className="ui-input"
                  inputMode="numeric"
                  value={tokenLimit}
                  onChange={(event) => setTokenLimit(event.target.value)}
                />
              </Field>
              <Field
                label={t("budgets.warnAt")}
                htmlFor="budget-warn"
                hint={t("budgets.warnAtHint")}
              >
                <input
                  id="budget-warn"
                  className="ui-input"
                  inputMode="numeric"
                  value={warnPercent}
                  onChange={(event) => setWarnPercent(event.target.value)}
                />
              </Field>
            </div>
            <div className="budget-form-actions">
              <Toggle
                id="budget-enabled"
                label={t("budgets.enabled")}
                checked={enabled}
                onChange={setEnabled}
              />
              <Button
                variant="primary"
                onClick={() => void handleSave()}
                disabled={saving}
              >
                {saving ? t("common.saving") : t("budgets.save")}
              </Button>
            </div>
          </div>
          {formError && (
            <p className="ui-field-error" role="alert">
              {formError}
            </p>
          )}
        </Card>

        <DataState
          labels={dataStateLabels(t)}
          loading={loading}
          error={error}
          isEmpty={data !== null && data.budgets.length === 0}
          onRetry={() => void load()}
          emptyFallback={
            <Card title={t("budgets.empty.title")}>
              <p className="ui-inline-note">
                {t("budgets.empty.body")}
              </p>
            </Card>
          }
        >
          {data && (
            <div className="grid-cards">
              {data.budgets.map((progress) => (
                <Card
                  key={progress.budget.id}
                  title={`${scopeLabel(progress, providers, t)} - ${t(`budgets.period${progress.budget.period.charAt(0).toUpperCase() + progress.budget.period.slice(1)}`)}`}
                  subtitle={t("budgets.periodStarted", { time: formatTimestamp(progress.period_start, language) })}
                  action={
                    <div className="row">
                      <Badge tone={stateTone(progress.state)}>
                        {t(`budgets.state.${progress.state}`)}
                      </Badge>
                      <Button
                        variant="danger"
                        size="small"
                        onClick={() => void handleDelete(progress.budget.id)}
                        aria-label={t("budgets.deleteAria", { id: String(progress.budget.id) })}
                      >
                        {t("common.remove")}
                      </Button>
                    </div>
                  }
                >
                  <div className="stack-tight">
                    <div className="bar-row">
                      <div className="bar-row-head">
                        <span>{t("budgets.cost")}</span>
                        <span className="ui-mono">
                          {progress.cost_used}
                          {progress.budget.cost_limit
                            ? ` / ${progress.budget.cost_limit}`
                            : t("budgets.noCostLimit")}
                        </span>
                      </div>
                      <ProgressBar
                        percent={progress.cost_percent}
                        tone={barTone(progress.state)}
                        label={t("budgets.costUsed")}
                      />
                    </div>
                    <div className="bar-row">
                      <div className="bar-row-head">
                        <span>{t("budgets.tokens")}</span>
                        <span className="ui-mono">
                          {formatCompact(progress.tokens_used)}
                          {progress.budget.token_limit
                            ? ` / ${formatCompact(progress.budget.token_limit)}`
                            : t("budgets.noTokenLimit")}
                        </span>
                      </div>
                      <ProgressBar
                        percent={progress.token_percent}
                        tone={barTone(progress.state)}
                        label={t("budgets.tokenUsed")}
                      />
                    </div>
                    <span className="ui-inline-note">
                      {t("budgets.requestsInPeriod", { count: String(progress.request_count) })}
                      {progress.unpriced_tokens > 0
                        ? t("budgets.unpricedNote", { tokens: formatCompact(progress.unpriced_tokens) })
                        : ""}
                    </span>
                    {!progress.budget.enabled && (
                      <Badge tone="neutral">{t("budgets.disabled")}</Badge>
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
