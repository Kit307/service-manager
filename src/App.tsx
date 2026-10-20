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
  serviceActive,
  type ServiceRef,
  type LogRecord,
  type Project,
  type ServiceDefinition,
  type Workspace,
} from "./lib/types";
import { Icon } from "./components/Icon";
import { ServiceEditor } from "./components/ServiceEditor";
import { LogPanel } from "./components/LogPanel";
import { AiConnections } from "./components/AiConnections";

import { ServiceSidebar } from "./components/ServiceSidebar";
import { GroupEditor } from "./components/GroupEditor";
import { GroupOverview, type ActionResult } from "./components/GroupOverview";
import {
  entries,
  refKey,
  serviceRef,
  type ServiceEntry,
} from "./lib/workspace";

const initial: Workspace = {
  projects: [],
  groups: [],
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
  const [view, setView] = useState<
    "workspace" | "group-settings" | "service" | "new" | "settings" | "ai"
  >("workspace");
  const [groupId, setGroupId] = useState("");
  const [results, setResults] = useState<ActionResult[]>([]);
  const [busy, setBusy] = useState(false);
  const [draftRevision, setDraftRevision] = useState("absent");
  const [error, setError] = useState("");
  const [logs, setLogs] = useState<LogRecord[]>([]);
  const [gap, setGap] = useState(false);
  const [theme, setTheme] = useState(
    () => window.localStorage.getItem("appearance") || "system",
  );
  const allServices = entries(workspace.projects);
  const group = workspace.groups.find((g) => g.id === groupId);
  const members = new Set(group?.services.map(refKey));
  const visibleServices = group
    ? allServices.filter((e) => members.has(refKey(serviceRef(e))))
    : allServices;
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
    const subscription = api
      .onWorkspace((data) => {
        if (live) setWorkspace(data);
      })
      .catch(() => () => {});
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
      if (inFlight) {
        pending = true;
        return;
      }
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
            while (
              start > 0 &&
              bytes + combined[start - 1].text.length * 3 + 160 <=
                2 * 1024 * 1024
            ) {
              bytes += combined[--start].text.length * 3 + 160;
            }
            return combined.slice(start);
          });
        }
      } catch (e) {
        if (live) setError(errorMessage(e));
      } finally {
        inFlight = false;
        if (live && pending) {
          clearTimeout(retry);
          retry = setTimeout(() => {
            void update();
          }, 100);
        }
      }
    };
    void update();
    const subscription = isDesktop()
      ? api
          .onLogs(() => {
            void update();
          })
          .catch(() => () => {})
      : Promise.resolve(() => {});
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
    if (next)
      setGroupId(
        workspace.groups.find((g) =>
          g.services.some((s) => s.projectId === p.id && s.serviceId === next),
        )?.id ?? "",
      );
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
  function overview(id = groupId) {
    setGroupId(id);
    setView("workspace");
    setResults([]);
    setError("");
    setProjectId("");
    setServiceId("");
    setLogs([]);
    setGap(false);
  }
  async function addService() {
    await perform(async () => {
      const path = await pickProjectFolder("Choose a service working folder");
      if (path) {
        const p = await api.openProject(path);
        select(p, "");
        beginEdit("new", p);
      }
    });
  }
  async function review(
    targets: ServiceEntry[],
    action: "start" | "stop" | "restart",
  ) {
    if (action === "stop") return true;
    const untrusted = targets.filter(
      ({ project: p, service: s }) => !p.trusted.includes(s.id),
    );
    for (const { project: p, service: s } of untrusted) {
      const command =
        s.type === "command" ? s.command : `Docker Compose: ${s.file}`;
      if (
        !(await confirmAction(
          `Trust and run this service?\n\n${command}\n\nFolder: ${p.path}/${s.cwd}\n\nThis command runs with your user permissions.`,
          "Trust this service?",
        ))
      )
        return false;
    }
    for (const { project: p, service: s } of untrusted)
      await api.trustService(p.id, s.id, p.revision);
    return true;
  }
  async function bulk(action: "start" | "stop" | "restart") {
    const targets = visibleServices.filter(
      ({ project: p, service: s }) =>
        action === "restart" ||
        (action === "stop"
          ? serviceActive(p.runtime[s.id]?.state)
          : !serviceActive(p.runtime[s.id]?.state)),
    );
    await perform(async () => {
      setResults([]);
      if (!(await review(targets, action))) return;
      const settled = await Promise.allSettled(
        targets.map(({ project: p, service: s }) =>
          api.control(p.id, s.id, action),
        ),
      );
      setResults(
        settled.map((result, i) => ({
          key: refKey(serviceRef(targets[i])),
          ...(result.status === "rejected"
            ? { error: errorMessage(result.reason) }
            : {}),
        })),
      );
    });
  }
  async function moveService(ref: ServiceRef, target?: string) {
    await perform(async () => {
      await api.moveService(ref, target);
      if (projectId === ref.projectId && serviceId === ref.serviceId)
        setGroupId(target ?? "");
    });
  }
  async function saveGroup(name: string, services: ServiceRef[]) {
    await perform(async () => {
      const saved = await api.saveGroup(group?.id, name, services);
      overview(saved.id);
    });
  }
  async function removeGroup() {
    if (
      !group ||
      !(await confirmAction(
        `Remove “${group.name}”? Its services will remain available under Ungrouped. No code files will be deleted.`,
        "Remove group?",
      ))
    )
      return;
    await perform(async () => {
      await api.removeGroup(group.id);
      overview("");
    });
  }
  async function control(action: "start" | "stop" | "restart") {
    if (!project || !service) return;
    await perform(async () => {
      if (!(await review([{ project, service }], action))) return;
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
      if (view === "new" && group) {
        try {
          await api.saveGroup(group.id, group.name, [
            ...group.services,
            { projectId: saved.id, serviceId: definition.id },
          ]);
        } catch (e) {
          await refresh();
          throw new Error(
            `Service saved, but could not add it to the group: ${errorMessage(e)}`,
            { cause: e },
          );
        }
      }
    });
  }
  async function remove() {
    if (
      !project ||
      !service ||
      !(await confirmAction(
        `Remove “${service.name}” from Service Manager? No code files will be deleted.`,
        "Remove service?",
      ))
    )
      return;
    await perform(async () => {
      await api.remove(project.id, service.id, project.revision);
      overview();
    });
  }

  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="Services">
        <div className="brand">
          <span className="brand-mark">
            <Icon name="terminal" size={18} />
          </span>
          <span>Service Manager</span>
          <span className="version">0.5</span>
        </div>
        <button
          className="open-folder"
          onClick={() => {
            void addService();
          }}
          disabled={busy}
        >
          <Icon name="plus" />
          Add service<span className="shortcut">↗</span>
        </button>
        <ServiceSidebar
          services={allServices}
          groups={workspace.groups}
          selected={
            view === "service" || view === "settings"
              ? refKey({ projectId, serviceId })
              : ""
          }
          groupId={view === "workspace" ? groupId : "__other"}
          busy={busy}
          onService={({ project: p, service: s }) => select(p, s.id)}
          onGroup={overview}
          onMove={(ref, target) => {
            void moveService(ref, target);
          }}
          onEdit={(id) => {
            setGroupId(id);
            setView("group-settings");
            setError("");
          }}
        />
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
              <Icon
                name={
                  theme === "dark"
                    ? "moon"
                    : theme === "light"
                      ? "sun"
                      : "monitor"
                }
                size={15}
              />
            </button>
          </div>
        </div>
      </aside>
      <main className="workspace">
        <header className="workspace-bar">
          <div className="breadcrumb">
            <Icon
              name={view === "ai" ? "ai" : group ? "group" : "grid"}
              size={14}
            />
            <span>
              {view === "ai"
                ? "AI Connections"
                : (group?.name ?? "All services")}
            </span>
            {service && (view === "service" || view === "settings") && (
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
        ) : view === "group-settings" ? (
          <GroupEditor
            key={groupId || "new-group"}
            group={group}
            groups={workspace.groups}
            services={allServices}
            busy={busy}
            onSave={(name, refs) => {
              void saveGroup(name, refs);
            }}
            onRemove={() => {
              void removeGroup();
            }}
            onCancel={() => overview()}
          />
        ) : view === "workspace" || !project ? (
          <GroupOverview
            name={group?.name ?? "All services"}
            services={visibleServices}
            busy={busy}
            results={results}
            onControl={(action) => {
              void bulk(action);
            }}
            onService={({ project: p, service: s }) => select(p, s.id)}
            onAdd={() => {
              void addService();
            }}
          />
        ) : view === "new" || view === "settings" ? (
          <div className="editor-scroll">
            <p className="editor-root">Working folder: {project.path}</p>
            <ServiceEditor
              key={view === "new" ? `new-${project.id}` : serviceId}
              service={view === "settings" ? service : undefined}
              saving={busy}
              onSave={(d) => {
                void save(d);
              }}
              onCancel={() => (service ? setView("service") : overview())}
              onRemove={
                view === "settings" && service && !active
                  ? () => {
                      void remove();
                    }
                  : undefined
              }
            />
          </div>
        ) : !service ? null : (
          <>
            <section className="service-summary">
              <div className="service-heading">
                <div className="service-identity">
                  <span className="service-icon">
                    <Icon
                      name={service.type === "compose" ? "stack" : "terminal"}
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
                      {`${project.path}/${service.cwd}`}
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
