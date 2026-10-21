import { useEffect, useRef, useState, type PointerEvent } from "react";
import { api, errorMessage } from "../lib/api";
import { serviceActive, stateLabel, type ServiceRef } from "../lib/types";
import { refKey, serviceRef, type ServiceEntry } from "../lib/workspace";
import { useServiceLogs } from "../lib/useServiceLogs";
import { LogPanel, type LogViewState } from "./LogPanel";
import { Icon } from "./Icon";
import "../styles/log-workspace.css";

type Pane = { id: string; tabs: ServiceRef[]; active?: string };
const storageKey = "log-workspace-v1";
const emptyPane = (): Pane => ({ id: crypto.randomUUID(), tabs: [] });
const defaultView = (): LogViewState => ({ stream: "all", query: "", follow: true });

function restored(): Pane[] {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(storageKey) ?? "null");
    if (!Array.isArray(raw)) return [emptyPane()];
    const seen = new Set<string>();
    const ids = new Set<string>();
    const panes: Pane[] = [];
    for (const value of raw) {
      if (!value || typeof value !== "object" || typeof value.id !== "string" || !value.id || ids.has(value.id) || !Array.isArray(value.tabs)) continue;
      ids.add(value.id);
      const tabs: ServiceRef[] = [];
      for (const ref of value.tabs) {
        if (!ref || typeof ref !== "object" || typeof ref.projectId !== "string" || !ref.projectId || typeof ref.serviceId !== "string" || !ref.serviceId) continue;
        const key = refKey(ref);
        if (!seen.has(key)) { tabs.push({ projectId: ref.projectId, serviceId: ref.serviceId }); seen.add(key); }
      }
      panes.push({ id: value.id, tabs, active: tabs.some((ref) => refKey(ref) === value.active) ? value.active : tabs[0] && refKey(tabs[0]) });
    }
    return panes.length ? panes : [emptyPane()];
  } catch { return [emptyPane()]; }
}

function Feed({ entry, view, onView, cursor, onCursor }: {
  entry: ServiceEntry; view: LogViewState; onView: (next: LogViewState) => void;
  cursor?: string; onCursor: (next: string) => void;
}) {
  const [clearing, setClearing] = useState(false);
  const [clearError, setClearError] = useState("");
  const logs = useServiceLogs(serviceRef(entry), cursor, onCursor);
  return <>
    {logs.error && <div className="log-feed-error" role="alert">{logs.error} <button onClick={logs.retry}>Retry</button></div>}
    {clearError && <div className="log-feed-error" role="alert">{clearError}</div>}
    <LogPanel compact logs={logs.logs} gap={logs.gap} active={serviceActive(entry.project.runtime[entry.service.id]?.state)}
      label={`${entry.service.name} logs`} viewState={view} onViewStateChange={onView} clearing={clearing} clearLabel="Clear output"
      onClear={() => {
        setClearing(true);
        setClearError("");
        void api.clearLogs(entry.project.id, entry.service.id).then(() => {
          logs.clearLocal();
        }).catch((error) => setClearError(errorMessage(error))).finally(() => setClearing(false));
      }} />
  </>;
}

function LogPane({ pane, services, visible, root, views, onView, cursors, onCursor, onActivate, onClose, onMove, onAdd, onDrag, onFocus, onClosePane }: {
  pane: Pane; services: ServiceEntry[]; visible: boolean; root: HTMLDivElement | null;
  views: Record<string, LogViewState>; onView: (key: string, next: LogViewState) => void;
  cursors: Record<string, string>; onCursor: (key: string, next: string) => void;
  onActivate: (ref: ServiceRef) => void; onClose: (ref: ServiceRef) => void;
  onMove: (ref: ServiceRef, offset: number) => void; onAdd: (ref: ServiceRef) => void;
  onDrag: (event: PointerEvent<HTMLButtonElement>, ref: ServiceRef) => void;
  onFocus: (key: string) => void; onClosePane: () => void;
}) {
  const element = useRef<HTMLElement>(null);
  const [inViewport, setInViewport] = useState(typeof IntersectionObserver === "undefined");
  useEffect(() => {
    if (typeof IntersectionObserver === "undefined" || !element.current) return;
    const observer = new IntersectionObserver(([entry]) => setInViewport(entry.isIntersecting), { root });
    observer.observe(element.current);
    return () => observer.disconnect();
  }, [root]);
  const selected = pane.tabs.find((ref) => refKey(ref) === pane.active);
  const entry = selected && services.find((service) => refKey(serviceRef(service)) === refKey(selected));
  function closeTab(ref: ServiceRef) {
    const workspace = element.current?.closest(".log-workspace");
    onClose(ref);
    requestAnimationFrame(() => {
      (element.current ?? workspace)?.querySelector<HTMLElement>('[role="tab"][aria-selected="true"], select[aria-label="Add service log"]')?.focus();
    });
  }
  return <section ref={element} className="log-workspace-pane" aria-label="Log pane" data-log-drop-pane={pane.id} onPointerDown={() => { if (pane.active) onFocus(pane.active); }} onFocus={(event) => {
    const key = event.target.closest<HTMLElement>("[data-log-drop-tab]")?.dataset.logDropTab ?? pane.active;
    if (key) onFocus(key);
  }}>
    <div className="log-workspace-pane-header" data-log-drop-pane={pane.id}>
    <div className="log-workspace-tabs" role="tablist" aria-label="Open service logs" data-log-drop-pane={pane.id}>
      {pane.tabs.map((ref, index) => {
        const key = refKey(ref);
        const item = services.find((value) => refKey(serviceRef(value)) === key);
        if (!item) return <div className="log-workspace-tab" key={key} data-log-drop-tab={key}>
          <button role="tab" aria-selected={pane.active === key} tabIndex={pane.active === key ? 0 : -1} onClick={() => onActivate(ref)}>Unavailable service</button>
          <button className="log-tab-close" aria-label="Close unavailable log tab" onClick={() => closeTab(ref)}><Icon name="close" size={12} /></button>
        </div>;
        const state = item.project.runtime[item.service.id]?.state ?? "stopped";
        const duplicate = services.some((other) => other !== item && other.service.name === item.service.name);
        return <div className={`log-workspace-tab ${pane.active === key ? "selected" : ""}`} key={key} data-log-drop-tab={key} data-log-drop-pane={pane.id}>
          <button role="tab" id={`log-tab-${encodeURIComponent(key)}`} aria-controls={`log-pane-${pane.id}`} aria-selected={pane.active === key} tabIndex={pane.active === key ? 0 : -1}
            aria-label={`${item.service.name}${duplicate ? ` · ${item.project.name}` : ""} ${stateLabel(state)}`}
            title={`${item.project.path} · Drag to reorder or another pane. Alt+Arrow moves a tab.`}
            onClick={() => onActivate(ref)} onPointerDown={(event) => onDrag(event, ref)}
            onKeyDown={(event) => {
              const offset = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
              if (event.altKey && offset) { event.preventDefault(); onMove(ref, offset); }
              else if (offset || event.key === "Home" || event.key === "End") {
                event.preventDefault();
                const next = event.key === "Home" ? 0 : event.key === "End" ? pane.tabs.length - 1 : (index + offset + pane.tabs.length) % pane.tabs.length;
                onActivate(pane.tabs[next]);
                const buttons = event.currentTarget.closest('[role="tablist"]')?.querySelectorAll<HTMLButtonElement>('[role="tab"]');
                buttons?.[next]?.focus();
              } else if (event.key === "Delete") { event.preventDefault(); closeTab(ref); }
            }}>
            <span className={`status-dot ${state}`} />{item.service.name}{duplicate && <small>{item.project.name}</small>}
          </button>
          <button className="log-tab-close" aria-label={`Close ${item.service.name} log tab`} title={`Close ${item.service.name} log tab`} onClick={() => closeTab(ref)}><Icon name="close" size={12} /></button>
        </div>;
      })}
    </div>
      <label className="log-add-select" title="Open a service log"><Icon name="plus" size={14} /><span aria-hidden="true">Open log</span><select aria-label="Add service log" value="" onChange={(event) => {
        const item = services.find((value) => refKey(serviceRef(value)) === event.target.value);
        if (item) onAdd(serviceRef(item));
      }}><option value="">Open log…</option>{services.map((item) => <option key={refKey(serviceRef(item))} value={refKey(serviceRef(item))}>{item.service.name} · {item.project.name}</option>)}</select></label>
      <button className="log-close-pane" aria-label="Close log pane" title="Close pane" onClick={onClosePane}><Icon name="close" size={14} /></button>
    </div>
    {entry && selected ? <div className="log-workspace-output" id={`log-pane-${pane.id}`} role="tabpanel" aria-label={`${entry.service.name} output`} aria-labelledby={`log-tab-${encodeURIComponent(refKey(selected))}`}>
      {visible && inViewport ? <Feed key={refKey(selected)} entry={entry} view={views[refKey(selected)] ?? defaultView()}
        onView={(next) => onView(refKey(selected), next)} cursor={cursors[refKey(selected)]} onCursor={(next) => onCursor(refKey(selected), next)} />
        : <div className="log-workspace-empty"><Icon name="terminal" size={22} /><strong>Observation paused</strong><p>Output resumes when this pane is visible.</p></div>}
    </div> : <div className="log-workspace-empty"><div className="log-empty-symbol"><Icon name="terminal" size={24} /></div><strong>A place for your output</strong><p>Open a service log or drop a tab here.</p></div>}
  </section>;
}

export function LogWorkspace({ services, requested, requestId = 0, visible = true, onSelect }: {
  services: ServiceEntry[]; requested?: ServiceRef; requestId?: number; visible?: boolean; onSelect?: (ref: ServiceRef) => void;
}) {
  const [panes, setPanes] = useState<Pane[]>(restored);
  const [views, setViews] = useState<Record<string, LogViewState>>({});
  const [cursors, setCursors] = useState<Record<string, string>>({});
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const [drop, setDrop] = useState("");
  const [dragging, setDragging] = useState(false);
  const [focused, setFocused] = useState<string>();
  const drag = useRef<{ ref: ServiceRef; x: number; y: number; active: boolean } | undefined>(undefined);
  const suppressClick = useRef(false);
  function activate(ref: ServiceRef, paneId?: string) {
    if (suppressClick.current) { suppressClick.current = false; return; }
    const key = refKey(ref);
    setFocused(key);
    setPanes((old) => {
      const existing = old.find((pane) => pane.tabs.some((tab) => refKey(tab) === key));
      const target = existing?.id ?? paneId ?? old[0].id;
      return old.map((pane) => pane.id === target ? { ...pane, tabs: existing ? pane.tabs : [...pane.tabs, ref], active: key } : pane);
    });
    onSelect?.(ref);
  }
  const requestedKey = requested && refKey(requested);
  const request = `${requestId}:${requestedKey ?? ""}`;
  const [lastRequest, setLastRequest] = useState("");
  if (request !== lastRequest) {
    setLastRequest(request);
    if (requestedKey) {
    setFocused(requestedKey);
    const ref: ServiceRef = { projectId: JSON.parse(requestedKey)[0], serviceId: JSON.parse(requestedKey)[1] };
    setPanes((old) => {
      const existing = old.find((pane) => pane.tabs.some((tab) => refKey(tab) === requestedKey));
      return old.map((pane, index) => pane.id === existing?.id || (!existing && index === 0)
        ? { ...pane, tabs: existing ? pane.tabs : [...pane.tabs, ref], active: requestedKey } : pane);
    });
    }
  }
  useEffect(() => {
    try { localStorage.setItem(storageKey, JSON.stringify(panes)); } catch { /* Layout still works for this session. */ }
  }, [panes]);
  const shown = panes;
  function close(ref: ServiceRef) {
    const key = refKey(ref);
    setPanes((old) => {
      const source = old.find((pane) => pane.tabs.some((tab) => refKey(tab) === key));
      const next = old.map((pane) => {
        const index = pane.tabs.findIndex((tab) => refKey(tab) === key);
        const tabs = pane.tabs.filter((tab) => refKey(tab) !== key);
        return { ...pane, tabs, active: pane.active === key ? tabs[Math.min(index, tabs.length - 1)] && refKey(tabs[Math.min(index, tabs.length - 1)]) : pane.active };
      }).filter((pane) => pane.tabs.length || pane.id !== source?.id);
      return next.length ? next : [emptyPane()];
    });
    setViews((old) => { const next = { ...old }; delete next[key]; return next; });
    setCursors((old) => { const next = { ...old }; delete next[key]; return next; });
  }
  function move(ref: ServiceRef, paneId: string, before?: string) {
    const key = refKey(ref);
    setFocused(key);
    setPanes((old) => {
      const target = paneId === "new" ? emptyPane() : old.find((pane) => pane.id === paneId);
      if (!target || before === key) return old;
      const source = old.find((pane) => pane.tabs.some((tab) => refKey(tab) === key));
      if (!source) return old;
      return [...old, ...(paneId === "new" ? [target] : [])].map((pane) => {
        const tabs = pane.tabs.filter((tab) => refKey(tab) !== key);
        if (pane.id === target.id) tabs.splice(before ? Math.max(0, tabs.findIndex((tab) => refKey(tab) === before)) : tabs.length, 0, ref);
        return { ...pane, tabs, active: pane.id === target.id ? key : pane.active === key ? tabs[0] && refKey(tabs[0]) : pane.active };
      }).filter((pane) => pane.tabs.length || pane.id !== source.id);
    });
  }
  function targetAt(event: PointerEvent) {
    return document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-log-drop-pane]");
  }
  function reorder(ref: ServiceRef, offset: number) {
    setPanes((old) => old.map((pane) => {
      const index = pane.tabs.findIndex((tab) => refKey(tab) === refKey(ref));
      const next = index + offset;
      if (index < 0 || next < 0 || next >= pane.tabs.length) return pane;
      const tabs = [...pane.tabs]; [tabs[index], tabs[next]] = [tabs[next], tabs[index]];
      return { ...pane, tabs };
    }));
  }
  const activeRefs = shown.flatMap((pane) => pane.tabs.filter((ref) => refKey(ref) === pane.active));
  const activeRef = activeRefs.find((ref) => refKey(ref) === focused) ?? activeRefs[0];
  return <div className={`log-workspace ${dragging ? "dragging-log" : ""}`} aria-label="Log workspace" hidden={!visible}
    onPointerMove={(event) => {
      const current = drag.current;
      if (!current || Math.hypot(event.clientX - current.x, event.clientY - current.y) < 5 && !current.active) return;
      current.active = true;
      setDragging(true);
      event.preventDefault();
      setDrop(targetAt(event)?.dataset.logDropPane ?? "");
    }}
    onPointerUp={(event) => {
      const current = drag.current;
      if (current?.active) {
        const target = targetAt(event);
        if (target?.dataset.logDropPane) move(current.ref, target.dataset.logDropPane, target.dataset.logDropTab);
        suppressClick.current = true;
      }
      drag.current = undefined; setDrop(""); setDragging(false);
    }} onPointerCancel={() => { drag.current = undefined; setDrop(""); setDragging(false); }}
    onKeyDown={(event) => { if (event.key === "Escape") { drag.current = undefined; setDrop(""); setDragging(false); } }}>
    <div className="log-workspace-toolbar">
      <div className="log-workspace-heading"><h1>Logs</h1><span>{shown.reduce((count, pane) => count + pane.tabs.length, 0)} tabs<span className="log-count-divider">/</span>{shown.length} panes</span></div>
      <div className="log-workspace-actions">
      {activeRef && <div className="log-arrange-controls"><button className="log-arrange-icon" aria-label="Move active log tab left" title="Move tab left · Alt+←" onClick={() => reorder(activeRef, -1)}><Icon name="chevron" size={13} style={{ transform: "rotate(180deg)" }} /></button><button className="log-arrange-icon" aria-label="Move active log tab right" title="Move tab right · Alt+→" onClick={() => reorder(activeRef, 1)}><Icon name="chevron" size={13} /></button></div>}
      {activeRef && <label className="log-move-select"><span aria-hidden="true">Move tab</span><Icon name="chevron" size={12} style={{ transform: "rotate(90deg)" }} /><select aria-label="Move active log tab to pane" value="" onChange={(event) => { if (event.target.value) move(activeRef, event.target.value); }}><option value="">Choose pane…</option>{shown.map((pane, index) => <option key={pane.id} value={pane.id}>Pane {index + 1}</option>)}<option value="new">New pane</option></select></label>}
      <button className="log-add-pane" onClick={() => setPanes((old) => [...old, emptyPane()])}><Icon name="split" size={15} />Add pane</button>
      </div>
    </div>
    <div className="log-workspace-grid" ref={setRoot}>
      {shown.map((pane) => <LogPane key={pane.id} pane={{ ...pane, active: pane.tabs.some((ref) => refKey(ref) === pane.active) ? pane.active : pane.tabs[0] && refKey(pane.tabs[0]) }} services={services} visible={visible} root={root}
        onFocus={setFocused} onClosePane={() => {
          setPanes((old) => { const remaining = old.filter((value) => value.id !== pane.id); return remaining.length ? remaining : [emptyPane()]; });
          const removed = new Set(pane.tabs.map(refKey));
          setViews((old) => Object.fromEntries(Object.entries(old).filter(([key]) => !removed.has(key))));
          setCursors((old) => Object.fromEntries(Object.entries(old).filter(([key]) => !removed.has(key))));
        }}
        views={views} onView={(key, next) => setViews((old) => ({ ...old, [key]: next }))} cursors={cursors} onCursor={(key, next) => setCursors((old) => old[key] === next ? old : ({ ...old, [key]: next }))}
        onActivate={(ref) => activate(ref, pane.id)} onClose={close} onMove={reorder} onAdd={(ref) => activate(ref, pane.id)} onDrag={(event, ref) => {
          if (event.button !== 0) return;
          suppressClick.current = false;
          drag.current = { ref, x: event.clientX, y: event.clientY, active: false };
          event.currentTarget.setPointerCapture?.(event.pointerId);
        }} />)}
      <div className={`log-workspace-new ${drop === "new" ? "drop-target" : ""}`} data-log-drop-pane="new">Drop a tab here to create a pane</div>
    </div>
  </div>;
}
