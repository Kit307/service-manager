import { useEffect, useRef, useState } from "react";
import { api, errorMessage, isDesktop } from "./api";
import type { LogRecord, ServiceRef } from "./types";

export function useServiceLogs(ref: ServiceRef, initialCursor?: string, onCursor?: (cursor: string) => void) {
  const [logs, setLogs] = useState<LogRecord[]>([]);
  const [gap, setGap] = useState(false);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const resume = useRef(initialCursor);
  const reportCursor = useRef(onCursor);
  useEffect(() => { reportCursor.current = onCursor; }, [onCursor]);
  useEffect(() => {
    let live = true;
    let cursor: string | undefined;
    let inFlight = false;
    let pending = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    const update = async () => {
      if (!live || !isDesktop()) return;
      if (inFlight) { pending = true; return; }
      inFlight = true;
      pending = false;
      try {
        const firstPage = cursor === undefined;
        const page = await api.logs(ref.projectId, ref.serviceId, cursor);
        if (!live) return;
        cursor = page.nextCursor;
        reportCursor.current?.(cursor);
        pending = pending || page.records.length > 0;
        setError("");
        setGap((old) => old || page.truncated);
        setLogs((old) => {
          const prior = firstPage ? [] : old;
          const seen = new Set(prior.map((line) => line.seq));
          const combined = [...prior, ...page.records.filter((line) => !seen.has(line.seq))].slice(-1000);
          let bytes = 0;
          let start = combined.length;
          while (start > 0 && bytes + combined[start - 1].text.length * 3 + 160 <= 2 * 1024 * 1024)
            bytes += combined[--start].text.length * 3 + 160;
          return combined.slice(start);
        });
      } catch (e) {
        if (live) setError(errorMessage(e));
      } finally {
        inFlight = false;
        if (live && pending) {
          clearTimeout(retry);
          retry = setTimeout(() => { void update(); }, 100);
        }
      }
    };
    // A resume probe detects eviction while a hidden pane released its buffer.
    const begin = async () => {
      if (resume.current && isDesktop()) {
        try {
          const page = await api.logs(ref.projectId, ref.serviceId, resume.current);
          if (live) setGap(page.truncated);
        } catch (e) { if (live) setError(errorMessage(e)); }
      }
      if (live) await update();
    };
    void begin();
    const subscription = isDesktop()
      ? api.onLogs(() => { void update(); }).catch(() => () => {})
      : Promise.resolve(() => {});
    return () => {
      live = false;
      clearTimeout(retry);
      void subscription.then((unsubscribe) => unsubscribe());
    };
  }, [ref.projectId, ref.serviceId, attempt]);
  return { logs, gap, error, retry: () => setAttempt((old) => old + 1), clearLocal: () => {
    resume.current = undefined;
    setLogs([]);
    setGap(false);
    setError("");
    setAttempt((old) => old + 1);
  } };
}
