import { useEffect, useMemo, useRef, useState } from "react";
import type { LogRecord } from "../lib/types";
import { Icon } from "./Icon";

export function LogPanel({
  logs,
  active,
  gap,
  onClear,
}: {
  logs: LogRecord[];
  active: boolean;
  gap: boolean;
  onClear: () => void;
}) {
  const [stream, setStream] = useState("all");
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
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
  return (
    <section className="log-panel" aria-label="Service logs">
      <div className="log-toolbar">
        <div className="log-title">
          <Icon name="terminal" />
          <h2>Output</h2>
          {active && <span className="live-indicator">Live</span>}
          <span className="log-count">
            {visible.length.toLocaleString()} lines
          </span>
        </div>
        <div className="button-row">
          <button
            className={follow ? "tiny active" : "tiny"}
            aria-pressed={follow}
            onClick={() => setFollow(!follow)}
          >
            <Icon name="arrow" size={13} />
            {follow ? "Following" : "Follow"}
          </button>
          <button
            className="tiny"
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
            {copied ? "Copied" : "Copy"}
          </button>
          <button className="tiny" onClick={onClear}>
            Clear
          </button>
        </div>
      </div>
      <div className="log-filters">
        <div className="segmented" aria-label="Output stream">
          {["all", "stdout", "stderr"].map((s) => (
            <button
              key={s}
              aria-pressed={stream === s}
              className={s === stream ? "selected" : ""}
              onClick={() => setStream(s)}
            >
              {s === "all" ? "All output" : s}
            </button>
          ))}
        </div>
        <label className="search-input">
          <Icon name="search" size={14} />
          <input
            aria-label="Search logs"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Find in output…"
          />
          <kbd>⌕</kbd>
        </label>
      </div>
      <div
        ref={viewport}
        className="log-viewport"
        role="region"
        aria-label="Log output"
        tabIndex={0}
        onScroll={() => {
          const el = viewport.current;
          if (el && el.scrollHeight - el.scrollTop - el.clientHeight > 40)
            setFollow(false);
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
            <div className={`log-line ${line.stream}`} key={line.seq}>
              <time>
                {new Date(line.timestamp).toLocaleTimeString("en-GB", {
                  hour12: false,
                })}
              </time>
              <span className="log-stream">
                {line.stream === "stderr"
                  ? "err"
                  : line.stream === "system"
                    ? "sys"
                    : "out"}
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
          {active ? "Receiving service output" : "No active stream"}
        </span>
        <span>Bounded history · plain text</span>
      </div>
    </section>
  );
}
