import { useState } from "react";
import type { Project, ServiceRef, Snapshot } from "../lib/types";
import { serviceActive } from "../lib/types";
import { refKey } from "../lib/workspace";
import "../styles/operations.css";

type Failure = { key: string; ref: ServiceRef; name: string; runId: string; error?: string };
function snapshots(projects: Project[]) {
  const current = new Map<string, Snapshot>();
  for (const project of projects) for (const service of project.config.services) {
    const snapshot = project.runtime[service.id];
    if (snapshot) current.set(refKey({ projectId: project.id, serviceId: service.id }), snapshot);
  }
  return current;
}
export function FailureAlerts({ projects, onService }: { projects: Project[]; onService: (ref: ServiceRef) => void }) {
  const [tracked, setTracked] = useState(() => ({ projects, snapshots: snapshots(projects), seen: new Set<string>(), alerts: [] as Failure[] }));
  if (tracked.projects !== projects) {
    const current = snapshots(projects);
    const seen = new Set(tracked.seen);
    const failures: Failure[] = [];
    for (const project of projects) for (const service of project.config.services) {
      const ref = { projectId: project.id, serviceId: service.id };
      const key = refKey(ref);
      const snapshot = current.get(key);
      const old = tracked.snapshots.get(key);
      const failureKey = JSON.stringify([key, snapshot?.runId]);
      if (snapshot?.state === "failed" && snapshot.runId && old && serviceActive(old.state) && old.state !== "stopping" && old.runId === snapshot.runId && !seen.has(failureKey)) {
        seen.add(failureKey);
        failures.push({ key: failureKey, ref, name: service.name, runId: snapshot.runId, error: snapshot.error });
      }
    }
    setTracked({ projects, snapshots: current, seen: new Set([...seen].slice(-200)), alerts: [...failures, ...tracked.alerts].slice(0, 20) });
  }
  const { alerts } = tracked;
  if (!alerts.length) return null;
  return (
    <aside className="failure-alerts" aria-label="Service failure alerts">
      <div className="operations-heading">
        <h2>Service failures ({alerts.length})</h2>
        <button className="tiny" onClick={() => setTracked((old) => ({ ...old, alerts: [] }))}>Dismiss all</button>
      </div>
      <div aria-live="polite" aria-relevant="additions">
        {alerts.map((alert) => (
          <div className="failure-alert" key={alert.key}>
            <div>
              <strong>{alert.name} failed</strong>
              <p>Run {alert.runId}{alert.error ? ` · ${alert.error}` : ""}</p>
            </div>
            <button className="tiny" onClick={() => onService(alert.ref)}>View logs</button>
            <button className="tiny" aria-label={`Dismiss failure for ${alert.name}`} onClick={() => setTracked((old) => ({
              ...old,
              alerts: old.alerts.filter((item) => item.key !== alert.key),
            }))}>Dismiss</button>
          </div>
        ))}
      </div>
    </aside>
  );
}
