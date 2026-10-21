import { serviceActive, stateLabel } from "../lib/types";
import { refKey, serviceRef, type ServiceEntry } from "../lib/workspace";
import { Icon } from "./Icon";
import "../styles/overview.css";
export type ActionResult = { key: string; error?: string };
export function GroupOverview({
  name,
  services,
  busy,
  results,
  onControl,
  onService,
  onAdd,
}: {
  name: string;
  services: ServiceEntry[];
  busy: boolean;
  results: ActionResult[];
  onControl: (action: "start" | "stop" | "restart") => void;
  onService: (entry: ServiceEntry) => void;
  onAdd: () => void;
}) {
  const active = services.filter(({ project, service }) =>
    serviceActive(project.runtime[service.id]?.state),
  ).length;
  const failed = services.filter(
    ({ project, service }) => project.runtime[service.id]?.state === "failed",
  ).length;
  return (
    <div className="workspace-overview">
      <div className="overview-content">
        <header className="overview-header">
          <div>
            <p className="welcome-kicker">Service workspace</p>
            <h1>{name}</h1>
          </div>
          <div className="overview-summary" role="group" aria-label="Workspace status">
            <span><strong>{services.length}</strong> services</span>
            <span>
              <span className="status-dot ready" />
              <strong>{active}</strong> active
            </span>
            {failed > 0 && (
              <span className="overview-failed">
                <span className="status-dot failed" />
                <strong>{failed}</strong> failed
              </span>
            )}
          </div>
        </header>
        <div className="overview-controls">
          <div className="button-row group-actions">
            <button
              className="primary"
              disabled={busy || active === services.length}
              onClick={() => onControl("start")}
            >
              <Icon name="play" />
              Run all
            </button>
            <button
              className="secondary"
              disabled={busy || !active}
              onClick={() => onControl("stop")}
            >
              <Icon name="stop" />
              Stop all
            </button>
            <button
              className="secondary"
              disabled={busy || !services.length}
              onClick={() => onControl("restart")}
            >
              <Icon name="restart" />
              Restart all
            </button>
          </div>
          <button className="secondary overview-add" disabled={busy} onClick={onAdd}>
            <Icon name="plus" />
            Add service
          </button>
        </div>
        <p className="overview-note">
          Runs in dependency order and reuses active services. Restart stops in reverse order. A failure cancels later starts.
        </p>
        <div className="group-service-list">
          {services.map((entry) => {
            const { project, service: s } = entry;
            const key = refKey(serviceRef(entry));
            const snapshot = project.runtime[s.id];
            const state = snapshot?.state ?? "stopped";
            const result = results.find((r) => r.key === key);
            const command = s.type === "command"
              ? s.command
              : `docker compose -f ${s.file} up`;
            const folder = `${project.path}/${s.cwd}`;
            return (
              <article className="group-service-card" key={key}>
                <div className="overview-service-heading">
                  <span className="overview-service-icon">
                    <Icon name={s.type === "command" ? "terminal" : "stack"} size={18} />
                  </span>
                  <h2 title={s.name}>{s.name}</h2>
                  <span className={`state-pill ${state}`}>
                    <span className={`status-dot ${state}`} />
                    {stateLabel(state)}
                  </span>
                </div>
                <div className="overview-service-details">
                  <code title={command}>{command}</code>
                  <p className="working-path" title={folder}>
                    {folder}
                  </p>
                </div>
                {(result?.error || snapshot?.error) && (
                  <p className="runtime-error" role="status">
                    {result?.error || snapshot?.error}
                  </p>
                )}
                {result && !result.error && (
                  <p className="overview-note">
                    Request accepted. Check live status above.
                  </p>
                )}
                <div className="overview-service-footer">
                  <span>{s.type === "command" ? "Command" : "Docker Compose"}</span>
                  <button
                    className="tiny"
                    aria-label={`View logs for ${s.name}`}
                    onClick={() => onService(entry)}
                  >
                    Open logs <Icon name="chevron" size={13} />
                  </button>
                </div>
              </article>
            );
          })}
          {!services.length && (
            <p className="empty-inline">
              No services here yet. Add a service or choose existing services in
              group settings. Adding a folder never runs its commands.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
