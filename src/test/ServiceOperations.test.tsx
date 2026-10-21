import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { api, confirmAction } from "../lib/api";
import { ServiceOperations } from "../components/ServiceOperations";
import { ServiceEditor } from "../components/ServiceEditor";
import type { Diagnostics, RunRecord, RunSummary } from "../lib/types";

vi.mock("../lib/api", async (original) => ({
  ...await original<typeof import("../lib/api")>(),
  confirmAction: vi.fn(),
  api: { inspectPorts: vi.fn(), killPort: vi.fn(), diagnose: vi.fn(), runHistory: vi.fn(), runDetails: vi.fn() },
}));
const run: RunSummary = { projectId: "fixture", serviceId: "one", runId: "run-1", command: "fixture command", startedAt: "2026-01-01T00:00:00Z", state: "failed", exitCode: 1, truncated: true };
const diagnostics: Diagnostics = { projectId: "fixture", serviceId: "one", state: { state: "failed", health: "notConfigured" }, summary: ["Process exited"], records: [], nextCursor: "", truncated: false, redacted: true };
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.inspectPorts).mockResolvedValue([{ port: 4321, pid: 42, name: "fixture-process", identity: "identity-42" }]);
  vi.mocked(api.diagnose).mockResolvedValue(diagnostics);
  vi.mocked(api.runHistory).mockResolvedValue([run]);
  vi.mocked(api.runDetails).mockResolvedValue({ ...run, records: [{ runId: "run-1", seq: 1, timestamp: run.startedAt, stream: "stderr", text: "selected output", truncated: false }, { runId: "other", seq: 2, timestamp: run.startedAt, stream: "stderr", text: "unrelated output", truncated: false }] });
});
it("canceled port confirmation does nothing", async () => {
  vi.mocked(confirmAction).mockResolvedValue(false);
  render(<ServiceOperations projectId="fixture" serviceId="one" mode="diagnostics" onService={vi.fn()} />);
  await userEvent.click(await screen.findByRole("button", { name: "Kill process" }));
  expect(confirmAction).toHaveBeenCalledWith(expect.stringContaining("PID 42"), expect.any(String));
  expect(api.killPort).not.toHaveBeenCalled();
});
it("keeps kill errors visible and refreshes listeners", async () => {
  vi.mocked(confirmAction).mockResolvedValue(true);
  vi.mocked(api.killPort).mockRejectedValue(new Error("Process identity changed"));
  render(<ServiceOperations projectId="fixture" serviceId="one" mode="diagnostics" onService={vi.fn()} />);
  await userEvent.click(await screen.findByRole("button", { name: "Kill process" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Process identity changed");
  expect(api.killPort).toHaveBeenCalledWith("fixture", "one", { port: 4321, pid: 42, identity: "identity-42" });
  await waitFor(() => expect(api.inspectPorts).toHaveBeenCalledTimes(2));
});
it("selects retained run logs and excludes records from other runs", async () => {
  render(<ServiceOperations projectId="fixture" serviceId="one" mode="history" onService={vi.fn()} />);
  await userEvent.click(await screen.findByRole("button", { name: /run-1/ }));
  await waitFor(() => expect(screen.getByRole("region", { name: "Historical run output" })).toHaveTextContent("selected output"));
  expect(screen.getByRole("region", { name: "Historical run output" })).not.toHaveTextContent("unrelated output");
  expect(screen.getByText(/Earlier output/)).toBeVisible();
});
it("ignores stale run details after switching services", async () => {
  let resolve!: (value: RunRecord) => void;
  vi.mocked(api.runDetails).mockReturnValue(new Promise((done) => { resolve = done; }));
  const view = render(<ServiceOperations projectId="fixture" serviceId="one" mode="history" onService={vi.fn()} />);
  await userEvent.click(await screen.findByRole("button", { name: /run-1/ }));
  view.rerender(<ServiceOperations projectId="fixture" serviceId="two" mode="history" onService={vi.fn()} />);
  await act(async () => resolve({ ...run, records: [{ runId: "run-1", seq: 1, timestamp: run.startedAt, stream: "stdout", text: "stale output", truncated: false }] }));
  expect(screen.queryByText("stale output")).not.toBeInTheDocument();
});
it("rejects partially valid or duplicate ports and preserves hidden dependencies", async () => {
  const save = vi.fn();
  render(<ServiceEditor service={{ id: "one", name: "One", cwd: ".", type: "command", command: "fixture", dependsOn: [{ projectId: "fixture", serviceId: "two" }] }} saving={false} onSave={save} onCancel={vi.fn()} />);
  const ports = screen.getByLabelText(/Ports/);
  await userEvent.type(ports, "3000,3000");
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent(/unique/);
  await userEvent.clear(ports);
  await userEvent.type(ports, "3000,invalid");
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).not.toHaveBeenCalled();
  await userEvent.clear(ports);
  await userEvent.type(ports, "3000,4321");
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenCalledWith(expect.objectContaining({ ports: [3000, 4321], dependsOn: [{ projectId: "fixture", serviceId: "two" }] }));
});

it("shows managed links and disables unknown listener termination", async () => {
  const select = vi.fn();
  vi.mocked(api.inspectPorts).mockResolvedValue([{ port: 4321, pid: 0, name: "Unknown listener", identity: "" }, { port: 4322, pid: 43, name: "managed-fixture", identity: "id-43", managed: { projectId: "fixture", serviceId: "two" } }]);
  render(<ServiceOperations projectId="fixture" serviceId="one" mode="diagnostics" onService={select} />);
  expect(await screen.findByRole("button", { name: "Kill process" })).toBeDisabled();
  await userEvent.click(screen.getByRole("button", { name: "View managed service" }));
  expect(select).toHaveBeenCalledWith({ projectId: "fixture", serviceId: "two" });
  expect(screen.getByRole("button", { name: "Stop managed service" })).toBeEnabled();
});
it("copies only after a user click and reports clipboard errors", async () => {
  const writeText = vi.fn().mockRejectedValue(new Error("Clipboard unavailable"));
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText } });
  render(<ServiceOperations projectId="fixture" serviceId="one" mode="diagnostics" onService={vi.fn()} />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Copy AI context" })).toBeEnabled());
  const preview = screen.getByRole("textbox", { name: "AI context preview" });
  expect(preview).toHaveAttribute("readonly");
  expect(writeText).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Copy AI context" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Clipboard unavailable");
});
it("ignores stale diagnostics after service selection changes", async () => {
  let resolve!: (value: typeof diagnostics) => void;
  vi.mocked(api.diagnose).mockReturnValueOnce(new Promise((done) => { resolve = done; })).mockResolvedValue({ ...diagnostics, serviceId: "two", summary: ["Current service only"] });
  const view = render(<ServiceOperations projectId="fixture" serviceId="one" mode="diagnostics" onService={vi.fn()} />);
  view.rerender(<ServiceOperations projectId="fixture" serviceId="two" mode="diagnostics" onService={vi.fn()} />);
  await screen.findByText("Current service only");
  await act(async () => resolve({ ...diagnostics, summary: ["Stale diagnostics"] }));
  expect(screen.queryByText("Stale diagnostics")).not.toBeInTheDocument();
});
it("rejects non-loopback Open URL before saving", async () => {
  const save = vi.fn();
  render(<ServiceEditor service={{ id: "one", name: "One", cwd: ".", type: "command", command: "fixture" }} saving={false} onSave={save} onCancel={vi.fn()} />);
  await userEvent.type(screen.getByLabelText(/Open URL/), "https://example.invalid");
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent("loopback");
});

it("confirms exact process and refreshes after successful termination", async () => {
  vi.mocked(confirmAction).mockResolvedValue(true);
  vi.mocked(api.killPort).mockResolvedValue(undefined);
  render(<ServiceOperations projectId="fixture" serviceId="one" mode="diagnostics" onService={vi.fn()} />);
  await userEvent.click(await screen.findByRole("button", { name: "Kill process" }));
  expect(confirmAction).toHaveBeenCalledWith(expect.stringMatching(/fixture-process, PID 42, TCP port 4321.*TERM.*5 seconds.*identity still matches/), "Kill port listener");
  await waitFor(() => expect(api.inspectPorts).toHaveBeenCalledTimes(2));
});
