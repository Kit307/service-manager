import { useEffect, useState } from "react";
import { stateLabel, type WorkspaceGroup } from "../lib/types";
import { refKey, serviceRef, type ServiceEntry } from "../lib/workspace";
import { Icon } from "./Icon";
export function ServiceSidebar({
  services,
  groups,
  selected,
  groupId,
  busy,
  onService,
  onGroup,
  onEdit,
}: {
  services: ServiceEntry[];
  groups: WorkspaceGroup[];
  selected: string;
  groupId: string;
  busy: boolean;
  onService: (entry: ServiceEntry) => void;
  onGroup: (id: string) => void;
  onEdit: (id: string) => void;
}) {
  const [collapsed, setCollapsed] = useState<string[]>(() => {
    try {
      const value: unknown = JSON.parse(
        localStorage.getItem("sidebar-collapsed") ?? "[]",
      );
      return Array.isArray(value)
        ? value.filter((x): x is string => typeof x === "string")
        : [];
    } catch {
      return [];
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem("sidebar-collapsed", JSON.stringify(collapsed));
    } catch {
      /* Disclosure still works without persistent storage. */
    }
  }, [collapsed]);
  const grouped = new Set(groups.flatMap((g) => g.services.map(refKey)));
  function row(entry: ServiceEntry) {
    const { project, service } = entry;
    const key = refKey(serviceRef(entry));
    const state = project.runtime[service.id]?.state ?? "stopped";
    return (
      <button
        key={key}
        aria-label={`${service.name} ${stateLabel(state)}`}
        className={`service-row ${selected === key ? "selected" : ""}`}
        title={`${project.path}/${service.cwd}`}
        onClick={() => onService(entry)}
      >
        <span className={`status-dot ${state}`} />
        <span>{service.name}</span>
        <span className="service-type">{stateLabel(state)}</span>
      </button>
    );
  }
  return (
    <>
      <button
        className={`ai-nav ${groupId === "" && !selected ? "selected" : ""}`}
        onClick={() => onGroup("")}
      >
        <Icon name="terminal" />
        All services<span className="group-count">{services.length}</span>
      </button>
      <div className="sidebar-label">
        Groups
        <button
          className="icon-button"
          aria-label="New group"
          disabled={busy}
          onClick={() => onEdit("")}
        >
          <Icon name="plus" size={14} />
        </button>
      </div>
      <nav className="project-tree" aria-label="Groups and services">
        {groups.map((g) => {
          const open = !collapsed.includes(g.id);
          const members = new Set(g.services.map(refKey));
          return (
            <section className="workspace-group" key={g.id}>
              <div className="project-header group-header">
                <button
                  className="icon-button disclosure"
                  aria-label={`${open ? "Collapse" : "Expand"} group ${g.name}`}
                  aria-expanded={open}
                  onClick={() =>
                    setCollapsed((old) =>
                      open ? [...old, g.id] : old.filter((id) => id !== g.id),
                    )
                  }
                >
                  <Icon
                    name="chevron"
                    size={12}
                    style={{ transform: open ? "rotate(90deg)" : undefined }}
                  />
                </button>
                <button
                  aria-label={g.name}
                  className={`project-row ${groupId === g.id && !selected ? "current" : ""}`}
                  onClick={() => onGroup(g.id)}
                >
                  <span>{g.name}</span>
                  <span className="group-count">
                    {
                      services.filter((e) => members.has(refKey(serviceRef(e))))
                        .length
                    }
                  </span>
                </button>
                <button
                  className="icon-button"
                  disabled={busy}
                  aria-label={`Edit group ${g.name}`}
                  onClick={() => onEdit(g.id)}
                >
                  <Icon name="settings" size={13} />
                </button>
              </div>
              {open && (
                <div className="group-members">
                  {services
                    .filter((e) => members.has(refKey(serviceRef(e))))
                    .map(row)}
                  {!g.services.length && (
                    <p className="empty-inline">No services yet.</p>
                  )}
                </div>
              )}
            </section>
          );
        })}
        {groups.length > 0 && <div className="sidebar-label">Ungrouped</div>}
        {services.filter((e) => !grouped.has(refKey(serviceRef(e)))).map(row)}
      </nav>
    </>
  );
}
