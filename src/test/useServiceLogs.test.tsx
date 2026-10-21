import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, isDesktop } from "../lib/api";
import type { LogPage } from "../lib/types";
import { useServiceLogs } from "../lib/useServiceLogs";

vi.mock("../lib/api", async (original) => ({ ...await original<typeof import("../lib/api")>(), isDesktop: vi.fn() }));
const ref = { projectId: "fixture", serviceId: "api" };
const page = (text: string, seq = 1): LogPage => ({ records: [{ seq, runId: "run", timestamp: "2026-01-01T00:00:00Z", stream: "stdout", text, truncated: false }], nextCursor: String(seq), truncated: false });
beforeEach(() => { vi.mocked(isDesktop).mockReturnValue(true); vi.spyOn(api, "onLogs").mockResolvedValue(() => {}); });
afterEach(() => vi.restoreAllMocks());

it("ignores a delayed response after the observer closes", async () => {
  let resolve!: (value: LogPage) => void;
  vi.spyOn(api, "logs").mockReturnValue(new Promise((done) => { resolve = done; }));
  const callback = vi.fn();
  const { unmount } = renderHook(() => useServiceLogs(ref, undefined, callback));
  unmount();
  await act(async () => resolve(page("late")));
  expect(callback).not.toHaveBeenCalled();
});

it("probes the prior cursor for retention loss then reloads a bounded retained tail", async () => {
  const logs = vi.spyOn(api, "logs").mockResolvedValueOnce({ records: [], nextCursor: "20", truncated: true }).mockResolvedValueOnce(page("retained", 30)).mockResolvedValue({ records: [], nextCursor: "30", truncated: false });
  const { result } = renderHook(() => useServiceLogs(ref, "10"));
  await waitFor(() => expect(result.current.logs[0]?.text).toBe("retained"));
  expect(logs.mock.calls.slice(0, 2)).toEqual([["fixture", "api", "10"], ["fixture", "api", undefined]]);
  expect(result.current.gap).toBe(true);
});

it("clearLocal invalidates in-flight reads and resets the cursor only after an explicit clear", async () => {
  let event!: () => void;
  vi.mocked(api.onLogs).mockImplementation(async (callback) => { event = callback; return () => {}; });
  let late!: (value: LogPage) => void;
  vi.spyOn(api, "logs").mockResolvedValueOnce(page("before")).mockImplementationOnce(() => new Promise((resolve) => { late = resolve; })).mockResolvedValue({ records: [], nextCursor: "2", truncated: false });
  const { result } = renderHook(() => useServiceLogs(ref));
  await waitFor(() => expect(result.current.logs).toHaveLength(1));
  await act(async () => event());
  await act(async () => result.current.clearLocal());
  await act(async () => late(page("stale", 2)));
  expect(result.current.logs).toEqual([]);
});

it("drains a queued event even when the in-flight response is empty", async () => {
  let event!: () => void;
  let resolve!: (value: LogPage) => void;
  vi.mocked(api.onLogs).mockImplementation(async (callback) => { event = callback; return () => {}; });
  const logs = vi.spyOn(api, "logs")
    .mockImplementationOnce(() => new Promise((done) => { resolve = done; }))
    .mockResolvedValueOnce(page("queued output", 2))
    .mockResolvedValue({ records: [], nextCursor: "2", truncated: false });
  const { result } = renderHook(() => useServiceLogs(ref));
  await act(async () => event());
  await act(async () => resolve({ records: [], nextCursor: "1", truncated: false }));
  await waitFor(() => expect(result.current.logs[0]?.text).toBe("queued output"));
  expect(logs).toHaveBeenNthCalledWith(2, "fixture", "api", "1");
});
