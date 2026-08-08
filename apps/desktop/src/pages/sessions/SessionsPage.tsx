import { useCallback, useEffect, useState } from "react";
import {
  Card,
  DataState,
  MetricCard,
  Table,
  Tabs,
  Toolbar,
} from "@lnwdeck/ui";
import {
  fetchSessions,
  renameProject,
  renameSession,
  type HistoryWindow,
  type ProjectUsage,
  type SessionsOverview,
  type SessionUsageRow,
} from "../../lib/native";
import { formatCompact, formatNumber, formatTimestamp } from "../../lib/freshness";
import { dataStateLabels, useI18n } from "../../lib/i18n";

const WINDOWS: Array<{ value: HistoryWindow; labelKey: string }> = [
  { value: "last_24h", labelKey: "costs.window24h" },
  { value: "last_7d", labelKey: "costs.window7d" },
  { value: "last_30d", labelKey: "costs.window30d" },
  { value: "all", labelKey: "costs.windowAll" },
];

/**
 * Session and project usage, grouped by folder.
 *
 * Every event is attributed to a project (folder) and a session through
 * privacy-safe keyed hashes; raw folder paths and session ids are never
 * stored. Display names are user-entered metadata; sessions without
 * attribution land in the "Unassigned" bucket.
 */
export function SessionsPage() {
  const { t, language } = useI18n();
  const [window, setWindow] = useState<HistoryWindow>("last_7d");
  const [provider, setProvider] = useState<string>("");
  const [data, setData] = useState<SessionsOverview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await fetchSessions(window, provider || undefined));
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

  const sessionCount = data
    ? data.projects.reduce((sum, project) => sum + project.sessions.length, 0)
    : 0;

  const handleRenameProject = useCallback(
    async (project: ProjectUsage, nextName: string) => {
      if (project.project_hash === "" || nextName === project.display_name) {
        return;
      }
      try {
        await renameProject(project.project_hash, nextName);
        await load();
      } catch (renameError) {
        setError(
          renameError instanceof Error ? renameError : new Error(String(renameError)),
        );
      }
    },
    [load],
  );

  const handleRenameSession = useCallback(
    async (session: SessionUsageRow, nextName: string) => {
      if (session.session_hash === "" || nextName === session.display_name) {
        return;
      }
      try {
        await renameSession(session.session_hash, nextName);
        await load();
      } catch (renameError) {
        setError(
          renameError instanceof Error ? renameError : new Error(String(renameError)),
        );
      }
    },
    [load],
  );

  return (
    <div>
      <div className="page-header">
        <div>
          <h2 className="page-title">{t("nav.sessions")}</h2>
          <p className="page-subtitle">{t("sessions.subtitle")}</p>
        </div>
      </div>

      <Toolbar label={t("sessions.filters")}>
        <Tabs
          label={t("sessions.windowLabel")}
          options={WINDOWS.map((window) => ({ value: window.value, label: t(window.labelKey) }))}
          value={window}
          onChange={setWindow}
        />
        <label className="ui-field-label" htmlFor="sessions-provider">
          {t("sessions.providerLabel")}
        </label>
        <select
          id="sessions-provider"
          className="ui-select"
          value={provider}
          onChange={(event) => setProvider(event.target.value)}
        >
          <option value="">{t("sessions.allProviders")}</option>
          {(data?.providers ?? []).map((id) => (
            <option key={id} value={id}>
              {id}
            </option>
          ))}
        </select>
      </Toolbar>

      <DataState
        labels={dataStateLabels(t)}
        loading={loading}
        error={error}
        isEmpty={data !== null && data.projects.length === 0}
        onRetry={() => void load()}
        emptyFallback={
          <Card title={t("sessions.empty.title")}>
            <p className="ui-inline-note">{t("sessions.empty.body")}</p>
          </Card>
        }
      >
        {data && (
          <div className="stack">
            <div className="grid-metrics">
              <MetricCard
                title={t("sessions.summaryProjects")}
                value={formatNumber(data.projects.length)}
              />
              <MetricCard
                title={t("sessions.summarySessions")}
                value={formatNumber(sessionCount)}
              />
              <MetricCard
                title={t("sessions.summaryTokens")}
                value={formatCompact(data.tokens_input + data.tokens_output)}
              />
              <MetricCard
                title={t("sessions.summaryCost")}
                value={data.cost}
              />
            </div>

            {data.projects.map((project) => (
              <ProjectCard
                key={project.project_hash === "" ? "__unassigned__" : project.project_hash}
                project={project}
                language={language}
                onRenameProject={handleRenameProject}
                onRenameSession={handleRenameSession}
              />
            ))}
          </div>
        )}
      </DataState>
    </div>
  );
}

interface ProjectCardProps {
  project: ProjectUsage;
  language: string;
  onRenameProject: (project: ProjectUsage, nextName: string) => void;
  onRenameSession: (session: SessionUsageRow, nextName: string) => void;
}

function ProjectCard({
  project,
  language,
  onRenameProject,
  onRenameSession,
}: ProjectCardProps) {
  const { t } = useI18n();
  const isUnassigned = project.project_hash === "";
  const title = isUnassigned
    ? t("sessions.unassigned")
    : project.display_name || t("sessions.unassigned");

  return (
    <Card
      title={title}
      subtitle={t("sessions.projectMeta", {
        requests: formatNumber(project.request_count),
        tokens: formatCompact(project.tokens_input + project.tokens_output),
        cost: project.cost,
      })}
      action={
        isUnassigned ? undefined : (
          <RenameControl
            label={t("sessions.renameProject")}
            placeholder={t("sessions.renameProjectPlaceholder")}
            value={project.display_name}
            onSave={(nextName) => onRenameProject(project, nextName)}
          />
        )
      }
    >
      <Table
        caption={t("sessions.tableCaption", { project: title })}
        headers={[
          t("sessions.colSession"),
          t("sessions.colProvider"),
          t("sessions.colRequests"),
          t("sessions.colInput"),
          t("sessions.colOutput"),
          t("sessions.colCost"),
          t("sessions.colLastUsed"),
          "",
        ]}
      >
        {project.sessions.map((session) => (
          <tr key={session.session_hash || "__unattributed__"}>
            <td>
              <div className="stack-tight">
                <span>{session.display_name || "—"}</span>
                {session.session_hash !== "" && (
                  <span className="ui-inline-note">{session.session_hash.slice(0, 10)}</span>
                )}
              </div>
            </td>
            <td>{session.provider_id}</td>
            <td className="ui-table-numeric">{formatNumber(session.request_count)}</td>
            <td className="ui-table-numeric">{formatCompact(session.tokens_input)}</td>
            <td className="ui-table-numeric">{formatCompact(session.tokens_output)}</td>
            <td className="ui-table-numeric">{session.cost}</td>
            <td>{formatTimestamp(session.last_seen_at, language)}</td>
            <td>
              {session.session_hash !== "" && (
                <RenameControl
                  label={t("sessions.renameSession")}
                  placeholder={t("sessions.renameSessionPlaceholder")}
                  value={session.display_name}
                  onSave={(nextName) => onRenameSession(session, nextName)}
                />
              )}
            </td>
          </tr>
        ))}
      </Table>
    </Card>
  );
}

function RenameControl({
  label,
  placeholder,
  value,
  onSave,
}: {
  label: string;
  placeholder: string;
  value: string;
  onSave: (nextName: string) => void;
}) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  if (!editing) {
    return (
      <button
        type="button"
        className="ui-button-ghost ui-button-small"
        aria-label={label}
        title={label}
        onClick={() => {
          setDraft(value);
          setEditing(true);
        }}
      >
        <RenameIcon />
      </button>
    );
  }

  return (
    <span
      role="group"
      aria-label={label}
      style={{ display: "inline-flex", gap: "0.35rem", alignItems: "center" }}
    >
      <input
        aria-label={label}
        className="ui-input"
        style={{ width: "13rem" }}
        placeholder={placeholder}
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            setEditing(false);
            onSave(draft.trim());
          } else if (event.key === "Escape") {
            setEditing(false);
          }
        }}
        autoFocus
      />
      <button
        type="button"
        className="ui-button-ghost ui-button-small"
        onClick={() => {
          setEditing(false);
          onSave(draft.trim());
        }}
      >
        {t("sessions.save")}
      </button>
    </span>
  );
}

function RenameIcon() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
      <path d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z" />
    </svg>
  );
}
