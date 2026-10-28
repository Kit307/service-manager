import { useEffect, useState } from "react";
import { api, errorMessage } from "./api";
import type { NetworkTraffic } from "./types";

type Activity = Record<string, { tx: boolean; rx: boolean }>;
type Direction = "tx" | "rx";

export function useNetworkTraffic(): { samples: NetworkTraffic[]; activity: Activity; loading: boolean; error: string | null } {
  const [samples, setSamples] = useState<NetworkTraffic[]>([]);
  const [activity, setActivity] = useState<Activity>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    let pollTimer: ReturnType<typeof setTimeout>;
    let previous = new Map<string, NetworkTraffic>();
    const timers = new Map<string, Partial<Record<Direction, ReturnType<typeof setTimeout>>>>();
    let current: Activity = {};

    const clearRoute = (key: string) => {
      const routeTimers = timers.get(key);
      if (routeTimers) { clearTimeout(routeTimers.tx); clearTimeout(routeTimers.rx); }
      timers.delete(key);
      delete current[key];
    };
    const pulse = (key: string, direction: Direction) => {
      const routeTimers = timers.get(key) ?? {};
      clearTimeout(routeTimers[direction]);
      current[key] = { ...(current[key] ?? { tx: false, rx: false }), [direction]: true };
      routeTimers[direction] = setTimeout(() => {
        current[key] = { ...current[key], [direction]: false };
        setActivity({ ...current });
      }, 1500);
      timers.set(key, routeTimers);
    };
    const poll = async () => {
      try {
        const next = await api.networkTraffic();
        if (!mounted) return;
        const nextPrevious = new Map<string, NetworkTraffic>();
        for (const sample of next) {
          const key = `${sample.projectId}/${sample.serviceId}/${sample.runId}/${sample.localPort}`;
          const before = previous.get(key);
          if (before && (sample.txBytes < before.txBytes || sample.rxBytes < before.rxBytes)) clearRoute(key);
          else if (before) {
            if (sample.txBytes > before.txBytes) pulse(key, "tx");
            if (sample.rxBytes > before.rxBytes) pulse(key, "rx");
          }
          nextPrevious.set(key, sample);
        }
        for (const key of previous.keys()) if (!nextPrevious.has(key)) clearRoute(key);
        previous = nextPrevious;
        setSamples(next);
        setActivity({ ...current });
        setError(null);
      } catch (cause) {
        if (!mounted) return;
        for (const key of timers.keys()) clearRoute(key);
        previous.clear();
        current = {};
        setSamples([]);
        setActivity({});
        setError(errorMessage(cause));
      } finally {
        if (mounted) {
          setLoading(false);
          pollTimer = setTimeout(poll, 1000);
        }
      }
    };
    void poll();
    return () => {
      mounted = false;
      clearTimeout(pollTimer);
      for (const key of timers.keys()) clearRoute(key);
    };
  }, []);

  return { samples, activity, loading, error };
}
