import { useCallback, useEffect, useState } from "react";
import { Badge, Button, Card, DataState, MetricCard } from "@lnwdeck/ui";
import {
  acknowledgeAlert,
  fetchAlerts,
  type AlertRowData,
  type AlertsViewData,
} from "../../lib/native";
import { formatTimestamp } from "../../lib/freshness";

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
          <h2 className="page-title">Alerts</h2>
          <p className="page-subtitle">
            Raised from stored quota reports, collector runs and budget
            progress. Providers that are simply not supported or not configured
            are not treated as failures.
          </p>
        </div>
        <Button onClick={() => void load()}>Re-evaluate</Button>
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
              <MetricCard title="Open" value={data.open_count} />
              <MetricCard
                title="Critical"
                value={data.critical_count}
                badge={
                  data.critical_count > 0 ? (
                    <Badge tone="danger">Needs attention</Badge>
                  ) : undefined
                }
              />
              <MetricCard
                title="Unacknowledged"
                value={data.unacknowledged_count}
              />
              <MetricCard title="Resolved records" value={resolved.length} />
            </div>

            <Card title="Open alerts">
              {data.open.length === 0 ? (
                <p className="ui-inline-note">
                  No alerts are open. Nothing crossed a quota threshold, no
                  collector failed, and no budget is over its limit.
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
                          First seen {formatTimestamp(alert.first_seen_at)}, last
                          seen {formatTimestamp(alert.last_seen_at)},{" "}
                          {alert.occurrences} occurrence(s)
                        </div>
                      </div>
                      <div className="row">
                        <Badge tone={severityTone(alert.severity)}>
                          {alert.severity}
                        </Badge>
                        {alert.acknowledged_at ? (
                          <Badge tone="neutral">Acknowledged</Badge>
                        ) : (
                          <Button
                            size="small"
                            onClick={() => void handleAcknowledge(alert.id)}
                          >
                            Acknowledge
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
                title="Resolved"
                subtitle="Conditions that no longer apply, kept for reference"
              >
                <div className="stack-tight">
                  {resolved.slice(0, 20).map((alert) => (
                    <div key={alert.id} className="alert-row">
                      <div className="alert-body">
                        <div className="alert-title">{alert.title}</div>
                        <div className="alert-detail">
                          Resolved {formatTimestamp(alert.resolved_at)}
                        </div>
                      </div>
                      <Badge tone="success">Resolved</Badge>
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
