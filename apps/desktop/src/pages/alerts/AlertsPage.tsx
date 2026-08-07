import { useCallback, useEffect, useState } from "react";
import { Badge, Button, Card, DataState, MetricCard } from "@lnwdeck/ui";
import {
  acknowledgeAlert,
  fetchAlerts,
  type AlertRowData,
  type AlertsViewData,
} from "../../lib/native";
import { formatTimestamp } from "../../lib/freshness";
import { useI18n } from "../../lib/i18n";

function severityTone(severity: AlertRowData["severity"]) {
  switch (severity) {
    case "critical":
      return "danger" as const;
    case "warning":
      return "warning" as const;
    default:
      return "info" as const;
  }
}

/**
 * Alerts raised from real quota, collector and budget state.
 *
 * With nothing wrong the page states that no alerts are open. It never renders
 * an "all systems normal" claim, because the absence of alerts is not the same
 * as a verified healthy system.
 */
export function AlertsPage() {
  const { t, language } = useI18n();
  const [data, setData] = useState<AlertsViewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await fetchAlerts());
    } catch (loadError) {
      setError(
        loadError instanceof Error ? loadError : new Error(String(loadError)),
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleAcknowledge = useCallback(
    async (id: number) => {
      setActionError(null);
      try {
        await acknowledgeAlert(id);
        await load();
      } catch (ackError) {
        setActionError(
          ackError instanceof Error ? ackError.message : String(ackError),
        );
      }
    },
    [load],
  );

  const resolved = (data?.history ?? []).filter(
    (alert) => alert.resolved_at !== null,
  );

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("nav.alerts")}</h2>
          <p className="page-subtitle">{t("alerts.subtitle")}</p>
        </div>
        <Button onClick={() => void load()}>{t("alerts.reevaluate")}</Button>
      </div>

      {actionError && (
        <p className="ui-field-error" role="alert">
          {actionError}
        </p>
      )}

      <DataState
        loading={loading}
        error={error}
        isEmpty={false}
        onRetry={() => void load()}
      >
        {data && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard title={t("alerts.open")} value={data.open_count} />
              <MetricCard
                title={t("alerts.critical")}
                value={data.critical_count}
                badge={
                  data.critical_count > 0 ? (
                    <Badge tone="danger">{t("alerts.needsAttention")}</Badge>
                  ) : undefined
                }
              />
              <MetricCard
                title={t("alerts.unacknowledged")}
                value={data.unacknowledged_count}
              />
              <MetricCard title={t("alerts.resolvedRecords")} value={resolved.length} />
            </div>

            <Card title={t("alerts.openTitle")}>
              {data.open.length === 0 ? (
                <p className="ui-inline-note">
                  {t("alerts.noneOpen")}
                </p>
              ) : (
                <div className="stack-tight">
                  {data.open.map((alert) => (
                    <div
                      key={alert.id}
                      className={`alert-row alert-row-${alert.severity}`}
                    >
                      <div className="alert-body">
                        <div className="alert-title">{alert.title}</div>
                        <div className="alert-detail">
                          {alert.detail}
                          {alert.error_code ? ` (${alert.error_code})` : ""}
                        </div>
                        <div className="alert-detail">
                          {t("alerts.firstLastSeen", { first: formatTimestamp(alert.first_seen_at, language), last: formatTimestamp(alert.last_seen_at, language), occurrences: String(alert.occurrences) })}
                        </div>
                      </div>
                      <div className="row">
                        <Badge tone={severityTone(alert.severity)}>
                          {t(`alerts.severity.${alert.severity}`)}
                        </Badge>
                        {alert.acknowledged_at ? (
                          <Badge tone="neutral">{t("alerts.acknowledged")}</Badge>
                        ) : (
                          <Button
                            size="small"
                            onClick={() => void handleAcknowledge(alert.id)}
                          >
                            {t("alerts.acknowledge")}
                          </Button>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </Card>

            {resolved.length > 0 && (
              <Card
                title={t("alerts.resolvedTitle")}
                subtitle={t("alerts.resolvedSubtitle")}
              >
                <div className="stack-tight">
                  {resolved.slice(0, 20).map((alert) => (
                    <div key={alert.id} className="alert-row">
                      <div className="alert-body">
                        <div className="alert-title">{alert.title}</div>
                        <div className="alert-detail">
                          {t("alerts.resolvedAt", { time: formatTimestamp(alert.resolved_at, language) })}
                        </div>
                      </div>
                      <Badge tone="success">{t("alerts.resolvedTitle")}</Badge>
                    </div>
                  ))}
                </div>
              </Card>
            )}
          </div>
        )}
      </DataState>
    </div>
  );
}
