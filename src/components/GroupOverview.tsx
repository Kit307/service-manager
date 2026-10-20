import { serviceActive, stateLabel } from "../lib/types";
import { refKey, serviceRef, type ServiceEntry } from "../lib/workspace";
import { Icon } from "./Icon";
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
  return (
    <div className="workspace-overview">
      <p className="welcome-kicker">Service workspace</p>
      <div className="overview-heading">
        <h1>{name}</h1>
        <span className="state-pill">
          {active} active · {services.length} services
        </span>
      </div>
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
        <button className="secondary" disabled={busy} onClick={onAdd}>
          <Icon name="plus" />
          Add service
        </button>
      </div>
      <p className="overview-note">
        Run all skips active services. Restart all restarts every service in
        this view. Check live status for progress.
      </p>
      <div className="group-service-list">
        {services.map((entry) => {
          const { project, service: s } = entry;
          const key = refKey(serviceRef(entry));
          const snapshot = project.runtime[s.id];
          const state = snapshot?.state ?? "stopped";
          const result = results.find((r) => r.key === key);
          return (
            <article className="group-service-card" key={key}>
              <div className="overview-heading">
                <h2>{s.name}</h2>
                <span className={`state-pill ${state}`}>
                  <span className={`status-dot ${state}`} />
                  {stateLabel(state)}
                </span>
              </div>
              <code>
                {s.type === "command"
                  ? s.command
                  : `docker compose -f ${s.file} up`}
              </code>
              <p className="working-path">
                {project.path}/{s.cwd}
              </p>
              <button className="tiny" onClick={() => onService(entry)}>
                View logs for {s.name}
              </button>
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
  );
}
