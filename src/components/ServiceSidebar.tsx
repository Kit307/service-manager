import { useEffect, useRef, useState, type PointerEvent } from "react";
import { stateLabel, type WorkspaceGroup, type ServiceRef } from "../lib/types";
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
  onMove,
}: {
  services: ServiceEntry[];
  groups: WorkspaceGroup[];
  selected: string;
  groupId: string;
  busy: boolean;
  onService: (entry: ServiceEntry) => void;
  onGroup: (id: string) => void;
  onEdit: (id: string) => void;
  onMove: (service: ServiceRef, groupId?: string) => void;
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
  const [dropTarget, setDropTarget] = useState<string>();
  const drag = useRef<
    | {
        pointerId: number;
        service: ServiceRef;
        x: number;
        y: number;
        active: boolean;
      }
    | undefined
  >(undefined);
  const suppressClick = useRef(false);
  function targetAt(event: PointerEvent<HTMLElement>) {
    return document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>("[data-service-drop]");
  }
  function cancelDrag() {
    drag.current = undefined;
    setDropTarget(undefined);
  }
  function movePointer(event: PointerEvent<HTMLElement>) {
    const current = drag.current;
    if (!current || current.pointerId !== event.pointerId) return;
    if (
      Math.hypot(event.clientX - current.x, event.clientY - current.y) < 5 &&
      !current.active
    )
      return;
    current.active = true;
    suppressClick.current = true;
    event.preventDefault();
    setDropTarget(targetAt(event)?.dataset.serviceDrop);
  }
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
        title={`${project.path}/${service.cwd} · Drag to a group or Ungrouped`}
        draggable={false}
        onPointerDown={(event) => {
          suppressClick.current = false;
          if (busy || event.button !== 0) return;
          drag.current = {
            pointerId: event.pointerId,
            service: serviceRef(entry),
            x: event.clientX,
            y: event.clientY,
            active: false,
          };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={movePointer}
        onPointerUp={(event) => {
          const current = drag.current;
          if (!current || current.pointerId !== event.pointerId) return;
          const target = current.active && !busy ? targetAt(event) : undefined;
          cancelDrag();
          if (current.active && !busy && target)
            onMove(current.service, target.dataset.serviceDrop || undefined);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") cancelDrag();
        }}
        onPointerCancel={cancelDrag}
        onLostPointerCapture={cancelDrag}
        onClick={() => {
          if (!suppressClick.current) onService(entry);
          suppressClick.current = false;
        }}
      >
        <span className="drag-grip">
          <Icon name="grip" size={12} />
        </span>
        <span className={`status-dot ${state}`} />
        <span className="service-name">{service.name}</span>
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
        <Icon name="grid" />
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
            <section
              role="region"
              aria-label={`Move service to group ${g.name}`}
              className={`workspace-group ${dropTarget === g.id ? "drop-active" : ""}`}
              key={g.id}
              data-service-drop={g.id}
            >
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
                  <Icon name="group" size={14} />
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
        <section
          role="region"
          aria-label="Move service to Ungrouped"
          className={`ungrouped-drop ${dropTarget === "" ? "drop-active" : ""}`}
          data-service-drop=""
        >
          <div className="sidebar-label">Ungrouped</div>
          {services.filter((e) => !grouped.has(refKey(serviceRef(e)))).map(row)}
          <p className="drop-hint">
            Drag a service here to remove it from a group.
          </p>
        </section>
      </nav>
    </>
  );
}
