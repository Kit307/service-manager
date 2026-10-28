import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import { useNetworkTraffic } from "../lib/useNetworkTraffic";

vi.mock("../lib/api", () => ({ api: { networkTraffic: vi.fn() }, errorMessage: (error: Error) => error.message }));

const row = (txBytes: number, rxBytes: number, runId = "run") => ({
  projectId: "project", serviceId: "service", runId, profileId: "vpn",
  localPort: 8080, remoteHost: "example.test", remotePort: 80,
  txBytes, rxBytes, activeConnections: 1,
});
const key = "project/service/run/8080";
const request = vi.mocked(api.networkTraffic);
const deferred = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

beforeEach(() => { vi.useFakeTimers(); request.mockReset(); });
afterEach(() => { vi.useRealTimers(); });

describe("useNetworkTraffic", () => {
  it("requests immediately and schedules one second after completion without overlap", async () => {
    const pending = deferred<ReturnType<typeof row>[]>();
    request.mockReturnValueOnce(pending.promise).mockResolvedValue([]);
    const { result, unmount } = renderHook(() => useNetworkTraffic());
    expect(request).toHaveBeenCalledTimes(1);
    expect(result.current.loading).toBe(true);
    await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
    expect(request).toHaveBeenCalledTimes(1);
    await act(async () => { pending.resolve([row(10, 20)]); await pending.promise; });
    expect(result.current.samples).toEqual([row(10, 20)]);
    expect(result.current.activity[key]).toBeUndefined();
    expect(result.current.loading).toBe(false);
    await act(async () => { await vi.advanceTimersByTimeAsync(999); });
    expect(request).toHaveBeenCalledTimes(1);
    await act(async () => { await vi.advanceTimersByTimeAsync(1); });
    expect(request).toHaveBeenCalledTimes(2);
    unmount();
  });

  it("expires and restarts each direction independently", async () => {
    request.mockResolvedValueOnce([row(10, 20)]).mockResolvedValueOnce([row(11, 20)])
      .mockResolvedValueOnce([row(11, 21)]).mockResolvedValueOnce([row(12, 21)])
      .mockResolvedValue([row(12, 21)]);
    const { result } = renderHook(() => useNetworkTraffic());
    await act(async () => {});
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.activity[key]).toEqual({ tx: true, rx: false });
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.activity[key]).toEqual({ tx: true, rx: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.activity[key]).toEqual({ tx: false, rx: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.activity[key]).toEqual({ tx: true, rx: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(500); });
    expect(result.current.activity[key]).toEqual({ tx: true, rx: false });
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.activity[key]).toEqual({ tx: false, rx: false });
  });

  it("clears baselines and activity on reset, run change, missing record and error, then recovers", async () => {
    request.mockResolvedValueOnce([row(10, 10)]).mockResolvedValueOnce([row(11, 11)])
      .mockResolvedValueOnce([row(1, 12)]).mockResolvedValueOnce([row(2, 13)])
      .mockResolvedValueOnce([row(50, 50, "new")]).mockResolvedValueOnce([])
      .mockResolvedValueOnce([row(100, 100, "new")]).mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce([row(200, 200, "new")]).mockResolvedValue([row(201, 201, "new")]);
    const { result } = renderHook(() => useNetworkTraffic());
    await act(async () => {});
    const tick = async () => { await act(async () => { await vi.advanceTimersByTimeAsync(1000); }); };
    await tick(); expect(result.current.activity[key]).toEqual({ tx: true, rx: true });
    await tick(); expect(result.current.activity[key]).toBeUndefined();
    await tick(); expect(result.current.activity[key]).toEqual({ tx: true, rx: true });
    await tick(); expect(result.current.activity["project/service/new/8080"]).toBeUndefined();
    expect(result.current.activity[key]).toBeUndefined();
    await tick(); expect(result.current.activity).toEqual({});
    await tick(); expect(result.current.activity).toEqual({});
    await tick(); expect(result.current.error).toBeTruthy(); expect(result.current.samples).toEqual([]);
    await tick(); expect(result.current.error).toBeNull(); expect(result.current.activity).toEqual({});
    await tick(); expect(result.current.activity["project/service/new/8080"]).toEqual({ tx: true, rx: true });
  });

  it("ignores a response after unmount and never schedules another request", async () => {
    const pending = deferred<ReturnType<typeof row>[]>();
    request.mockReturnValue(pending.promise);
    const { result, unmount } = renderHook(() => useNetworkTraffic());
    unmount();
    await act(async () => { pending.resolve([row(1, 1)]); await pending.promise; await vi.advanceTimersByTimeAsync(5000); });
    expect(result.current.samples).toEqual([]);
    expect(request).toHaveBeenCalledTimes(1);
  });
});
