import { useCallback, useEffect, useState } from "react";
import {
  api,
  confirmAction,
  errorMessage,
  isDesktop,
  pickProjectFolder,
} from "./lib/api";
import {
  stopped,
  type LogRecord,
  type Project,
  type ServiceDefinition,
  type Workspace,
} from "./lib/types";
import { Icon } from "./components/Icon";
import { ServiceEditor } from "./components/ServiceEditor";
import { LogPanel } from "./components/LogPanel";
import { AiConnections } from "./components/AiConnections";

const initial: Workspace = {
  projects: [],
  ai: { enabled: false, endpoint: "http://127.0.0.1:47831/mcp", clients: [] },
  uncleanShutdown: false,
};
const labels: Record<string, string> = {
  stopped: "Stopped",
  starting: "Starting",
  running: "Running",
  ready: "Ready",
  stopping: "Stopping",
  failed: "Failed",
};
export default function App() {
  const [workspace, setWorkspace] = useState(initial);
  const [projectId, setProjectId] = useState("");
  const [serviceId, setServiceId] = useState("");
  const [view, setView] = useState<"service" | "new" | "settings" | "ai">(
    "service",
  );
  const [busy, setBusy] = useState(false);
  const [draftRevision, setDraftRevision] = useState("absent");
  const [error, setError] = useState("");
  const [logs, setLogs] = useState<LogRecord[]>([]);
  const [gap, setGap] = useState(false);
  const [theme, setTheme] = useState(
    () => window.localStorage.getItem("appearance") || "system",
  );
  const project = workspace.projects.find((p) => p.id === projectId);
  const service = project?.config.services.find((s) => s.id === serviceId);
  const snapshot = project?.runtime[serviceId] ?? stopped;
  const active = ["starting", "running", "ready", "stopping"].includes(
    snapshot.state,
  );
  const runningCount = workspace.projects.reduce(
    (n, p) =>
      n +
      Object.values(p.runtime).filter((s) =>
        ["starting", "running", "ready"].includes(s.state),
      ).length,
    0,
  );
  const refresh = useCallback(async () => {
    const data = await api.workspace();
    setWorkspace(data);
    return data;
  }, []);
  useEffect(() => {
    let live = true;
    const update = async () => {
      try {
        const data = await api.workspace();
        if (live) setWorkspace(data);
      } catch (e) {
        if (live) setError(errorMessage(e));
      }
    };
    void update();
    if (!isDesktop())
      return () => {
        live = false;
      };
    const subscription = api.onWorkspace((data) => { if (live) setWorkspace(data); }).catch(() => () => {});
    return () => {
      live = false;
      void subscription.then((unsubscribe) => unsubscribe());
    };
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    window.localStorage.setItem("appearance", theme);
  }, [theme]);
  useEffect(() => {
    let live = true;
    let cursor: string | undefined;
    let inFlight = false;
    let pending = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const update = async () => {
      if (!projectId || !serviceId || !live || !isDesktop()) return;
      if (inFlight) { pending = true; return; }
      inFlight = true;
      pending = false;
      try {
        const page = await api.logs(projectId, serviceId, cursor);
        if (live) {
          cursor = page.nextCursor;
          pending = page.records.length > 0;
          setGap((old) => old || page.truncated);
          setLogs((old) => {
            const combined = [
              ...old,
              ...page.records.filter(
                (line) => !old.some((x) => x.seq === line.seq),
              ),
            ].slice(-1000);
            let bytes = 0;
            let start = combined.length;
            while (start > 0 && bytes + combined[start - 1].text.length * 3 + 160 <= 2 * 1024 * 1024) { bytes += combined[--start].text.length * 3 + 160; }
            return combined.slice(start);
          });
        }
      } catch (e) {
        if (live) setError(errorMessage(e));
      } finally {
        inFlight = false;
        if (live && pending) { clearTimeout(retry); retry = setTimeout(() => { void update(); }, 100); }
      }
    };
    void update();
    const subscription = isDesktop() ? api.onLogs(() => { void update(); }).catch(() => () => {}) : Promise.resolve(() => {});
    return () => {
      live = false;
      clearTimeout(retry);
      void subscription.then((unsubscribe) => unsubscribe());
    };
  }, [projectId, serviceId]);

  function select(p: Project, id?: string) {
    const next = id ?? p.config.services[0]?.id ?? "";
    if (projectId !== p.id || serviceId !== next) {
      setLogs([]);
      setGap(false);
    }
    setProjectId(p.id);
    setServiceId(next);
    setView("service");
    setError("");
  }
  function beginEdit(mode: "new" | "settings", p = project) {
    if (p) {
      setDraftRevision(p.revision);
      setView(mode);
    }
  }
  async function perform(action: () => Promise<unknown>) {
    setBusy(true);
    setError("");
    try {
      await action();
      await refresh();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }
  async function openFolder() {
    await perform(async () => {
      const path = await pickProjectFolder();
      if (path) {
        const p = await api.openProject(path);
        select(p);
      }
    });
  }
  async function control(action: "start" | "stop" | "restart") {
    if (!project || !service) return;
    await perform(async () => {
      if (action !== "stop" && !project.trusted.includes(service.id)) {
        const command =
          service.type === "command"
            ? service.command
            : `Docker Compose: ${service.file}`;
        if (
          !(await confirmAction(
            `Trust and run this service?\n\n${command}\n\nFolder: ${project.path}/${service.cwd}\n\nThis command runs with your user permissions. Opening a project never grants trust automatically.`,
            "Trust this service?",
          ))
        )
          return;
        await api.trustService(project.id, service.id, project.revision);
      }
      await api.control(project.id, service.id, action);
    });
  }
  async function save(definition: ServiceDefinition) {
    if (!project) return;
    await perform(async () => {
      const saved = await api.saveService(
        project.id,
        definition,
        draftRevision,
      );
      setProjectId(saved.id);
      setServiceId(definition.id);
      setView("service");
    });
  }
  async function remove() {
    if (
      !project ||
      !service ||
      !(await confirmAction(
        `Remove “${service.name}” from this project's configuration? No project files will be deleted.`,
        "Remove service?",
      ))
    )
      return;
    await perform(async () => {
      await api.remove(project.id, service.id, project.revision);
      const data = await refresh();
      const p = data.projects.find((p) => p.id === project.id);
      if (p) select(p);
    });
  }

  async function removeProject(p: Project) {
    const composeNote = p.config.services.some((s) => s.type === "compose")
      ? "\n\nDocker containers and volumes will be kept. Opening this folder again creates a new managed stack; existing Docker data is not reused automatically."
      : "";
    if (!(await confirmAction(
      `Remove “${p.name}” from Service Manager?\n\nYour code folder and saved service configuration will be kept. You can open the folder again later. Unsaved edits to this project will be discarded.${composeNote}`,
      "Remove project?",
    ))) return;
    await perform(async () => {
      await api.removeProject(p.id);
      const data = await refresh();
      if (projectId === p.id) {
        if (data.projects[0]) select(data.projects[0]);
        else {
          setProjectId("");
          setServiceId("");
          setLogs([]);
          setGap(false);
          setView("service");
        }
      }
    });
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Projects and services">
        <div className="brand">
          <span className="brand-mark">
            <Icon name="terminal" size={18} />
          </span>
          <span>Service Manager</span>
          <span className="version">0.3</span>
        </div>
        <button
          className="open-folder"
          onClick={() => {
            void openFolder();
          }}
          disabled={busy}
        >
          <Icon name="folder" />
          Open Folder<span className="shortcut">↗</span>
        </button>
        <div className="sidebar-label">
          Workspace<span>{workspace.projects.length}</span>
        </div>
        <nav className="project-tree">
          {workspace.projects.map((p) => (
            <div className="project-group" key={p.id}>
              <div className="project-header">
              <button
                className={`project-row ${projectId === p.id && view !== "ai" ? "current" : ""}`}
                onClick={() => select(p)}
                title={p.path}
              >
                <Icon
                  name="chevron"
                  size={12}
                  style={{ transform: "rotate(90deg)" }}
                />
                <Icon name="folder" size={15} />
                <span>{p.name}</span>
              </button>
              <button
                className="icon-button remove-project"
                aria-label={`Remove project ${p.name}`}
                title={Object.values(p.runtime).some((s) => ["starting", "running", "ready", "stopping"].includes(s.state)) ? "Stop all services before removing this project" : "Remove project from Service Manager"}
                disabled={busy || Object.values(p.runtime).some((s) => ["starting", "running", "ready", "stopping"].includes(s.state))}
                onClick={() => { void removeProject(p); }}
              >
                <Icon name="close" size={14} />
              </button>
              </div>
              {p.config.services.map((s) => {
                const state = p.runtime[s.id]?.state ?? "stopped";
                return (
                  <button
                    key={s.id}
                    className={`service-row ${projectId === p.id && serviceId === s.id && view === "service" ? "selected" : ""}`}
                    onClick={() => select(p, s.id)}
                  >
                    <span className={`status-dot ${state}`} />
                    <span>{s.name}</span>
                    <span className="service-type">{labels[state]}</span>
                  </button>
                );
              })}
              <button
                className="add-service-row"
                onClick={() => {
                  select(p);
                  beginEdit("new", p);
                }}
              >
                <Icon name="plus" size={13} />
                Add service
              </button>
            </div>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <button
            className={`ai-nav ${view === "ai" ? "selected" : ""}`}
            onClick={() => setView("ai")}
          >
            <Icon name="ai" />
            AI Connections
            <span
              className={`connection-dot ${workspace.ai.enabled ? "connected" : ""}`}
            />
          </button>
          <div className="sidebar-footer">
            <span>
              <span
                className={`status-dot ${runningCount ? "running" : "stopped"}`}
              />
              {runningCount} running
            </span>
            <button
              className="icon-button"
              title={`Appearance: ${theme}. Click to change.`}
              aria-label="Change appearance"
              onClick={() =>
                setTheme(
                  theme === "system"
                    ? "dark"
                    : theme === "dark"
                      ? "light"
                      : "system",
                )
              }
            >
              <Icon name="sun" size={15} />
            </button>
          </div>
        </div>
      </aside>
      <main className="workspace">
        <header className="workspace-bar">
          <div className="breadcrumb">
            <Icon name={view === "ai" ? "ai" : "folder"} size={14} />
            <span>
              {view === "ai"
                ? "AI Connections"
                : (project?.name ?? "Workspace")}
            </span>
            {service && view !== "ai" && (
              <>
                <span className="slash">/</span>
                <span className="breadcrumb-current">{service.name}</span>
              </>
            )}
          </div>
          <span className="local-badge">
            <span className="connection-dot connected" />
            Local workspace
          </span>
        </header>
        {error && (
          <div className="error-banner" role="alert">
            <span>{error}</span>
            <button
              className="icon-button"
              aria-label="Dismiss error"
              onClick={() => setError("")}
            >
              <Icon name="close" size={14} />
            </button>
          </div>
        )}
        {workspace.uncleanShutdown && (
          <div className="warning-banner">
            The previous session ended unexpectedly. Check for leftover
            processes before running services again.
          </div>
        )}
        {view === "ai" ? (
          <AiConnections
            workspace={workspace}
            onRefresh={refresh}
            onError={(e) => setError(errorMessage(e))}
          />
        ) : !project ? (
          <section className="welcome">
            <div className="welcome-icon">
              <Icon name="folder" size={30} />
            </div>
            <p className="welcome-kicker">A workspace for what you run.</p>
            <h1>
              Your services.
              <br />
              One place to run them.
            </h1>
            <p>
              Open a code folder, add your commands,
              <br />
              and keep every service and its output in view.
            </p>
            <button
              className="primary large"
              onClick={() => {
                void openFolder();
              }}
              disabled={busy}
            >
              <Icon name="folder" />
              Open Folder
            </button>
            <span className="safe-note">
              <Icon name="shield" size={13} />
              Opening a folder never runs its commands.
            </span>
            {!isDesktop() && (
              <span className="browser-note">
                Native folder access is available in the desktop app.
              </span>
            )}
          </section>
        ) : view === "new" || view === "settings" ? (
          <div className="editor-scroll">
            <ServiceEditor
              key={view === "new" ? `new-${project.id}` : serviceId}
              service={view === "settings" ? service : undefined}
              saving={busy}
              onSave={(d) => {
                void save(d);
              }}
              onCancel={() => setView("service")}
              onRemove={
                view === "settings" && service && !active
                  ? () => {
                      void remove();
                    }
                  : undefined
              }
            />
          </div>
        ) : !service ? (
          <section className="welcome project-empty">
            <span className="welcome-icon">
              <Icon name="terminal" size={30} />
            </span>
            <h1>
              What does this
              <br />
              project run?
            </h1>
            <p>
              Add a frontend, an API, or a Compose stack.
              <br />
              Your commands stay with this project.
            </p>
            <button className="primary large" onClick={() => beginEdit("new")}>
              <Icon name="plus" />
              Add your first service
            </button>
            <span className="safe-note">
              Configuration saves to .service-manager/project.json
            </span>
          </section>
        ) : (
          <>
            <section className="service-summary">
              <div className="service-heading">
                <div className="service-identity">
                  <span className="service-icon">
                    <Icon
                      name={service.type === "compose" ? "folder" : "terminal"}
                      size={24}
                    />
                  </span>
                  <div>
                    <div className="title-with-state">
                      <h1>{service.name}</h1>
                      <span className={`state-pill ${snapshot.state}`}>
                        <span className={`status-dot ${snapshot.state}`} />
                        {labels[snapshot.state]}
                      </span>
                    </div>
                    <p>
                      {service.type === "compose"
                        ? "Docker Compose"
                        : "Shell command"}
                      <span>·</span>
                      {service.cwd === "." ? "Project root" : service.cwd}
                      {snapshot.pid && (
                        <>
                          <span>·</span>PID {snapshot.pid}
                        </>
                      )}
                    </p>
                  </div>
                </div>
                <div className="button-row">
                  <button
                    className="secondary"
                    disabled={busy || !active || snapshot.state === "stopping"}
                    onClick={() => {
                      void control("restart");
                    }}
                  >
                    <Icon name="restart" />
                    Restart
                  </button>
                  {active ? (
                    <button
                      className="stop-button"
                      disabled={busy || snapshot.state === "stopping"}
                      onClick={() => {
                        void control("stop");
                      }}
                    >
                      <Icon name="stop" />
                      {snapshot.state === "stopping" ? "Stopping…" : "Stop"}
                    </button>
                  ) : (
                    <button
                      className="primary"
                      disabled={busy}
                      onClick={() => {
                        void control("start");
                      }}
                    >
                      <Icon name="play" />
                      Run service
                    </button>
                  )}
                </div>
              </div>
              <div className="command-display">
                <span className="command-prompt">$</span>
                <code>
                  {service.type === "command"
                    ? service.command
                    : `docker compose -f ${service.file} up`}
                </code>
                <button
                  className="icon-button"
                  title="Edit service"
                  aria-label="Edit service"
                  onClick={() => beginEdit("settings")}
                >
                  <Icon name="settings" size={15} />
                </button>
              </div>
              <div className="service-meta">
                <span>
                  <Icon name="shield" size={12} />
                  {project.trusted.includes(service.id)
                    ? "Trusted configuration"
                    : "Review required before first run"}
                </span>
                <span>
                  {snapshot.activity ||
                    (snapshot.exitCode !== undefined
                      ? `Last exit code: ${snapshot.exitCode}`
                      : "Ready when you are")}
                </span>
              </div>
              {snapshot.error && (
                <p className="runtime-error" role="status">
                  {snapshot.error}
                </p>
              )}
            </section>
            <div className="service-tabs">
              <button className="selected">
                <Icon name="terminal" size={14} />
                Output
              </button>
              <button onClick={() => beginEdit("settings")}>
                <Icon name="settings" size={14} />
                Settings
              </button>
              <span>
                {service.healthCheck
                  ? `Health check · ${snapshot.health}`
                  : "No health check configured"}
              </span>
            </div>
            <LogPanel
              key={`${project.id}/${service.id}`}
              logs={logs}
              active={active}
              gap={gap}
              onClear={() => {
                void perform(async () => {
                  await api.clearLogs(project.id, service.id);
                  setLogs([]);
                });
              }}
            />
          </>
        )}
      </main>
    </div>
  );
}
