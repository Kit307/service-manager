import { useEffect, useMemo, useRef, useState } from "react";
import type { LogRecord } from "../lib/types";
import { Icon } from "./Icon";
import "../styles/log-panel.css";

// shortcut: supports JSON string levels and prefixed console levels; extend for demonstrated logger formats.
function severity(text: string): "error" | "warning" | "info" | "unknown" {
  let level: unknown;
  if (text.trimStart().startsWith("{")) {
    try {
      const value: unknown = JSON.parse(text);
      if (value && typeof value === "object" && "level" in value)
        level = value.level;
    } catch {
      return "unknown";
    }
  } else {
    // Only a level at the start (after an optional timestamp) is a severity hint.
    level = text
      .trimStart()
      .match(
        /^(?:\d{4}-\d{2}-\d{2}[T ][\d:.+-]+Z?\s+)?\[?(ERROR|ERR|FATAL|PANIC|WARNING|WARN|WRN|INFO|INF)\]?(?=\s|:|$)/i,
      )?.[1];
  }
  if (typeof level !== "string") return "unknown";
  switch (level.toLowerCase()) {
    case "error":
    case "err":
    case "fatal":
    case "panic":
      return "error";
    case "warning":
    case "warn":
    case "wrn":
      return "warning";
    case "info":
    case "inf":
      return "info";
    default:
      return "unknown";
  }
}

export type LogViewState = {
  stream: "all" | "stdout" | "stderr";
  query: string;
  follow: boolean;
};

export function LogPanel({
  logs,
  active,
  gap,
  onClear,
  viewState,
  onViewStateChange,
  label = "Service logs",
  clearing = false,
  clearLabel = "Clear",
  compact = false,
}: {
  logs: LogRecord[];
  active: boolean;
  gap: boolean;
  onClear: () => void;
  viewState?: LogViewState;
  onViewStateChange?: (next: LogViewState) => void;
  label?: string;
  clearing?: boolean;
  clearLabel?: string;
  compact?: boolean;
}) {
  const [localViewState, setLocalViewState] = useState<LogViewState>({
    stream: "all",
    query: "",
    follow: true,
  });
  const { stream, query, follow } = viewState ?? localViewState;
  function updateViewState(change: Partial<LogViewState>) {
    const next = { stream, query, follow, ...change };
    if (!viewState) setLocalViewState(next);
    onViewStateChange?.(next);
  }
  const [copied, setCopied] = useState(false);
  const viewport = useRef<HTMLDivElement>(null);
  const visible = useMemo(
    () =>
      logs.filter(
        (line) =>
          (stream === "all" || line.stream === stream) &&
          line.text.toLowerCase().includes(query.toLowerCase()),
      ),
    [logs, query, stream],
  );
  useEffect(() => {
    if (follow && viewport.current)
      viewport.current.scrollTop = viewport.current.scrollHeight;
  }, [visible, follow]);
  const search = <label className="search-input">
    <Icon name="search" size={14} />
    <input aria-label="Search logs" value={query}
      onChange={(e) => updateViewState({ query: e.target.value })}
      placeholder={compact ? "Search logs…" : "Find in output…"} />
    {!compact && <kbd>⌕</kbd>}
  </label>;
  return (
    <section className={`log-panel ops-console${compact ? " log-panel-compact" : ""}`} aria-label={label}>
      <div className="log-toolbar">
        {compact ? <>
          {search}
          <label className="log-stream-picker">
            <span>{stream === "all" ? "All streams" : stream}</span>
            <Icon name="chevron" size={11} />
            <select aria-label="Output stream" value={stream}
              onChange={(event) => updateViewState({ stream: event.target.value as LogViewState["stream"] })}>
              <option value="all">All output</option>
              <option value="stdout">stdout</option>
              <option value="stderr">stderr</option>
            </select>
          </label>
        </> : <div className="log-title">
          <Icon name="terminal" />
          <h2>Output</h2>
          {active && <span className="live-indicator">Live</span>}
          <span className="log-count">
            {visible.length.toLocaleString()} lines
          </span>
        </div>}
        <div className="button-row">
          <button
            className={follow ? "tiny active" : "tiny"}
            aria-pressed={follow}
            title={follow ? "Pause following" : "Follow output"}
            onClick={() => updateViewState({ follow: !follow })}
          >
            <Icon name="arrow" size={13} />
            <span className="log-action-label">{follow ? "Following" : "Follow"}</span>
          </button>
          <button
            className="tiny"
            title={copied ? "Copied" : "Copy output"}
            onClick={async () => {
              try {
                await navigator.clipboard.writeText(
                  visible.map((l) => l.text).join("\n"),
                );
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              } catch {
                setCopied(false);
              }
            }}
          >
            <Icon name={copied ? "check" : "copy"} size={13} />
            <span className="log-action-label">{copied ? "Copied" : "Copy"}</span>
          </button>
          <button className="tiny" onClick={onClear} disabled={clearing} title={clearLabel}>
            {compact && <Icon name="trash" size={13} />}
            <span className="log-action-label">{clearLabel}</span>
          </button>
        </div>
      </div>
      {!compact && <div className="log-filters">
        <div className="segmented" aria-label="Output stream">
          {(["all", "stdout", "stderr"] as const).map((s) => (
            <button
              key={s}
              aria-pressed={stream === s}
              className={s === stream ? "selected" : ""}
              onClick={() => updateViewState({ stream: s })}
            >
              {s === "all" ? "All output" : s}
            </button>
          ))}
        </div>
        {search}
      </div>}
      <div
        ref={viewport}
        className="log-viewport"
        role="region"
        aria-label="Log output"
        tabIndex={0}
        onScroll={() => {
          const el = viewport.current;
          if (
            follow &&
            el &&
            el.scrollHeight - el.scrollTop - el.clientHeight > 40
          )
            updateViewState({ follow: false });
        }}
      >
        {gap && (
          <div className="log-gap">
            Earlier output is no longer retained. Showing available history.
          </div>
        )}
        {!visible.length ? (
          <div className="log-empty">
            <Icon name="terminal" size={22} />
            <p>
              {query
                ? "No matching output"
                : active
                  ? "Waiting for output…"
                  : "Your next run starts here"}
            </p>
            <span>
              {query
                ? "Try another search or stream filter."
                : "Run this service to see stdout and stderr in real time."}
            </span>
          </div>
        ) : (
          visible.map((line) => (
            <div
              className={`log-line ${line.stream} severity-${severity(line.text)}`}
              key={line.seq}
            >
              <time>
                {new Date(line.timestamp).toLocaleTimeString("en-GB", {
                  hour12: false,
                })}
              </time>
              <span
                className="log-stream"
                title={
                  line.stream === "stderr"
                    ? "Standard error stream; this may include informational logs."
                    : `Output stream: ${line.stream}`
                }
              >
                {line.stream}
              </span>
              <pre>
                {line.text}
                {line.truncated && " … [truncated]"}
              </pre>
            </div>
          ))
        )}
      </div>
      <div className="log-status">
        <span>
          <span className={`status-dot ${active ? "running" : "stopped"}`} />
          {compact ? active ? "Live output" : "Not running" : active ? "Receiving service output" : "No active stream"}
        </span>
        <span>{compact ? `${visible.length.toLocaleString()} lines` : "Bounded history · plain text"}</span>
      </div>
    </section>
  );
}
