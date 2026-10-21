import { useEffect, useRef, useState } from "react";
import { api, confirmAction, errorMessage } from "../lib/api";
import type { Diagnostics, PortListener, RunRecord, RunSummary, ServiceRef } from "../lib/types";
import "../styles/operations.css";

type Props = { projectId: string; serviceId: string; mode: "history" | "diagnostics"; onService: (ref: ServiceRef) => void };
export function ServiceOperations(props: Props) {
  return <OperationsContent key={JSON.stringify([props.projectId, props.serviceId, props.mode])} {...props} />;
}
function OperationsContent({ projectId, serviceId, mode, onService }: Props) {
  const [history, setHistory] = useState<RunSummary[]>([]);
  const [run, setRun] = useState<RunRecord>();
  const [selected, setSelected] = useState("");
  const [ports, setPorts] = useState<PortListener[]>([]);
  const [diagnostics, setDiagnostics] = useState<Diagnostics>();
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const alive = useRef(true);
  const selection = useRef(0);
  const refreshVersion = useRef(0);
  function refresh() {
    const version = ++refreshVersion.current;
    const current = () => alive.current && version === refreshVersion.current;
    const read = mode === "history"
      ? api.runHistory(projectId, serviceId).then((result) => {
          if (current()) setHistory(result.filter((item) => item.projectId === projectId && item.serviceId === serviceId).sort((a, b) => b.startedAt.localeCompare(a.startedAt)));
        })
      : Promise.allSettled([api.inspectPorts(projectId, serviceId), api.diagnose(projectId, serviceId)]).then((results) => {
          if (!current()) return;
          if (results[0].status === "fulfilled") setPorts(results[0].value);
          if (results[1].status === "fulfilled") setDiagnostics(results[1].value);
          const failures = results.filter((result) => result.status === "rejected");
          if (failures.length) setError(failures.map((result) => errorMessage(result.reason)).join(" · "));
        });
    return read.catch((cause) => {
      if (current()) setError(errorMessage(cause));
    }).finally(() => {
      if (current()) setLoading(false);
    });
  }
  useEffect(() => {
    alive.current = true;
    void refresh();
    return () => { alive.current = false; };
    // The keyed parent remounts this view whenever its request target changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  async function selectRun(runId: string) {
    const request = ++selection.current;
    setSelected(runId);
    setRun(undefined);
    setError("");
    try {
      const result = await api.runDetails(projectId, serviceId, runId);
      if (alive.current && request === selection.current && result.runId === runId && result.projectId === projectId && result.serviceId === serviceId) setRun(result);
    } catch (cause) {
      if (alive.current && request === selection.current) setError(errorMessage(cause));
    }
  }
  async function kill(listener: PortListener) {
    setBusy(true);
    try {
      const confirmed = await confirmAction(
        `${listener.managed ? "Stop managed service" : "Kill process"}: ${listener.name}, PID ${listener.pid}, TCP port ${listener.port}. Send TERM, then after 5 seconds force termination only if process identity still matches.`,
        listener.managed ? "Stop managed service" : "Kill port listener",
      );
      if (!confirmed || !alive.current) return;
      setError("");
      try {
        await api.killPort(projectId, serviceId, { port: listener.port, pid: listener.pid, identity: listener.identity });
      } finally {
        if (alive.current) await refresh();
      }
    } catch (cause) {
      if (alive.current) setError(errorMessage(cause));
    } finally {
      if (alive.current) setBusy(false);
    }
  }
  const context = diagnostics ? JSON.stringify({
    projectId, serviceId, runId: diagnostics.runId,
    state: diagnostics.state.state,
    summary: diagnostics.summary.slice(0, 20).map((line) => line.slice(0, 400)),
    records: diagnostics.records.slice(-50).map((record) => ({ ...record, text: record.text.slice(0, 400) })),
    nextCursor: diagnostics.nextCursor, redacted: diagnostics.redacted,
    truncated: diagnostics.truncated || diagnostics.summary.length > 20 || diagnostics.summary.some((line) => line.length > 400) || diagnostics.records.length > 50 || diagnostics.records.some((record) => record.text.length > 400),
  }, null, 2) : "";
  return (
    <section className="operations-panel" aria-label={mode === "history" ? "Run history" : "Service diagnostics"}>
      <div className="operations-heading">
        <h2>{mode === "history" ? "Run history" : "Diagnostics"}</h2>
        <button className="secondary" disabled={loading || busy} onClick={() => {
          setError("");
          setLoading(true);
          void refresh();
        }}>Refresh</button>
      </div>
      {error && <p className="operations-error" role="alert">{error}</p>}
      {loading && <p role="status">Loading…</p>}
      {mode === "history" ? (
        <>
          {!loading && !history.length && <p>No retained runs yet.</p>}
          <div className="run-list">
            {history.map((item) => (
              <button key={item.runId} aria-pressed={selected === item.runId} onClick={() => void selectRun(item.runId)}>
                <strong>{item.runId}</strong>
                <span>{item.state} · {new Date(item.startedAt).toLocaleString()}</span>
                <code>{item.command}</code>
              </button>
            ))}
          </div>
          {selected && !run && !error && <p role="status">Loading run…</p>}
          {run && (
            <section aria-label="Selected run">
              <h3>{run.runId}</h3>
              <dl className="run-metadata">
                <dt>Command</dt><dd><code>{run.command}</code></dd>
                <dt>Started</dt><dd>{new Date(run.startedAt).toLocaleString()}</dd>
                <dt>Ended</dt><dd>{run.endedAt ? new Date(run.endedAt).toLocaleString() : "Not ended"}</dd>
                <dt>State</dt><dd>{run.state}</dd>
                <dt>Exit code</dt><dd>{run.exitCode ?? "—"}</dd>
              </dl>
              {run.error && <p>{run.error}</p>}
              {run.truncated && <p className="log-gap">Earlier output is no longer retained. Showing available history.</p>}
              <div className="historical-output" role="region" aria-label="Historical run output" tabIndex={0}>
                {run.records.filter((record) => record.runId === selected).map((record) => (
                  <pre key={record.seq}><time>{record.timestamp}</time> {record.stream} {record.text}{record.truncated && " … [truncated]"}</pre>
                ))}
              </div>
            </section>
          )}
        </>
      ) : (
        <>
          <section aria-label="Port listeners">
            <h3>Port listeners</h3>
            {!loading && !ports.length && <p>No listeners found on configured ports.</p>}
            {ports.map((listener) => (
              <div className="port-listener" key={`${listener.port}:${listener.pid}:${listener.identity}`}>
                <div>
                  <strong>TCP {listener.port}</strong>
                  <span>{listener.name} · PID {listener.pid || "unknown"}</span>
                  {listener.managed && (
                    <button className="tiny" onClick={() => onService(listener.managed!)}>View managed service</button>
                  )}
                </div>
                <button className="danger-text" disabled={busy || listener.pid <= 1 || !listener.identity} onClick={() => void kill(listener)}>
                  {listener.managed ? "Stop managed service" : "Kill process"}
                </button>
              </div>
            ))}
          </section>
          <section aria-label="Local diagnostics observations">
            <h3>Local diagnostics observations</h3>
            <p>Deterministic local checks and bounded, redacted output. Review the context before sharing it.</p>
            {diagnostics && <ul>{diagnostics.summary.map((line, index) => <li key={index}>{line}</li>)}</ul>}
            <p>Ask connected Codex or Claude to use the MCP <code>diagnose_service</code> tool for analysis.</p>
            <label className="context-label">
              AI context preview
              <textarea readOnly rows={12} value={context} />
            </label>
            <button className="secondary" disabled={!context} onClick={async () => {
              setCopied(false);
              try {
                await navigator.clipboard.writeText(context);
                if (alive.current) setCopied(true);
              } catch (cause) {
                if (alive.current) setError(`Could not copy context: ${errorMessage(cause)}`);
              }
            }}>Copy AI context</button>
            {copied && <span role="status">Copied</span>}
          </section>
        </>
      )}
    </section>
  );
}
