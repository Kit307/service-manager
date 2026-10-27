import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import App from "../App";
import { api } from "../lib/api";
import type { Workspace } from "../lib/types";

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  return {
    ...original,
    confirmAction: vi.fn().mockResolvedValue(true),
    api: {
      ...original.api,
      workspace: vi.fn(),
      control: vi.fn(),
      controlSequence: vi.fn(),
      cancelSequence: vi.fn(),
      openUrl: vi.fn(),
      runHistory: vi.fn(),
      inspectPorts: vi.fn(),
      diagnose: vi.fn(),
      moveService: vi.fn(),
    },
  };
});
let workspace: Workspace;
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  workspace = {
    projects: [{
      id: "fixture", name: "Fixture", path: "/fixture", revision: "r1", trusted: ["one", "two"],
      config: { schemaVersion: 1, services: [
        { id: "one", name: "One", type: "command", command: "fixture command", cwd: ".", url: "http://127.0.0.1:4321" },
        { id: "two", name: "Two", type: "command", command: "second command", cwd: "." },
      ] },
      runtime: { one: { state: "running", runId: "run-1", health: "notConfigured" } },
    }],
    groups: [{ id: "group", name: "Fixture group", services: [{ projectId: "fixture", serviceId: "one" }, { projectId: "fixture", serviceId: "two" }] }],
    ai: { enabled: false, endpoint: "", clients: [] }, uncleanShutdown: false,
  };
  vi.mocked(api.workspace).mockImplementation(async () => workspace);
  vi.mocked(api.controlSequence).mockResolvedValue(undefined);
  vi.mocked(api.cancelSequence).mockResolvedValue(undefined);
  vi.mocked(api.openUrl).mockResolvedValue(undefined);
  vi.mocked(api.runHistory).mockResolvedValue([]);
  vi.mocked(api.inspectPorts).mockResolvedValue([]);
  vi.mocked(api.diagnose).mockResolvedValue({ projectId: "fixture", serviceId: "one", state: { state: "running", health: "notConfigured" }, summary: ["Fixture running"], records: [], nextCursor: "", truncated: false, redacted: true });
});
it("cancels a pending sequence without stopping already started services", async () => {
  let reject!: (cause: Error) => void;
  vi.mocked(api.controlSequence).mockReturnValueOnce(new Promise((_resolve, fail) => { reject = fail; }));
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: "Fixture group" }));
  await userEvent.click(screen.getByRole("button", { name: "Run all services" }));
  const cancel = await screen.findByRole("button", { name: "Cancel sequence" });
  expect(cancel).toBeEnabled();
  await userEvent.click(cancel);
  expect(api.cancelSequence).toHaveBeenCalledTimes(1);
  expect(api.control).not.toHaveBeenCalled();
  expect(api.controlSequence).toHaveBeenCalledTimes(1);
  await act(async () => reject(new Error("Sequence canceled; started services remain running")));
  expect(await screen.findByRole("alert")).toHaveTextContent("Sequence canceled");
  expect(workspace.projects[0].runtime.one.state).toBe("running");
});
it("retains service identity and controls across History and Diagnostics tabs", async () => {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: /One Running/ }));
  await userEvent.click(screen.getByRole("button", { name: "History" }));
  expect(await screen.findByRole("region", { name: "Run history" })).toBeVisible();
  expect(screen.getByRole("heading", { level: 1, name: "One" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Restart" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Stop" })).toBeVisible();
  await userEvent.click(screen.getByRole("button", { name: "Diagnostics" }));
  expect(await screen.findByRole("region", { name: "Service diagnostics" })).toBeVisible();
  expect(screen.getByRole("heading", { level: 1, name: "One" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Stop" })).toBeVisible();
  await userEvent.click(screen.getByRole("button", { name: "Output" }));
  expect(screen.getByRole("region", { name: "Service logs" })).toBeVisible();
});
it("opens the configured service URL through the native API", async () => {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: /One Running/ }));
  await userEvent.click(screen.getByRole("button", { name: "Open URL" }));
  await waitFor(() => expect(api.openUrl).toHaveBeenCalledExactlyOnceWith("fixture", "one"));
});

it("opens full service output from the sidebar and retains separate workspace tabs", async () => {
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("button", { name: /One Running/ });
  expect(screen.queryByRole("tab", { name: "One Running" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /One Running/ }));
  expect(screen.getByRole("region", { name: "Service logs" })).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Open in log workspace" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "Add service log" }), JSON.stringify(["fixture", "two"]));
  expect(screen.getAllByRole("tab")).toHaveLength(2);
  await user.click(screen.getByRole("button", { name: /One Running/ }));
  expect(screen.getByRole("region", { name: "Service logs" })).toBeVisible();
  expect(screen.getByRole("heading", { level: 1, name: "One" })).toBeVisible();
  expect(screen.queryByRole("tab", { name: "One Running" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Open in log workspace" }));
  expect(screen.getAllByRole("tab")).toHaveLength(2);
  expect(screen.getByRole("tab", { name: "One Running" })).toHaveAttribute("aria-selected", "true");
  await user.click(screen.getByRole("button", { name: /Two Stopped/ }));
  expect(screen.getByRole("heading", { level: 1, name: "Two" })).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Log workspace" }));
  expect(screen.getAllByRole("tab")).toHaveLength(2);
  expect(api.control).not.toHaveBeenCalled();
  expect(api.controlSequence).not.toHaveBeenCalled();
});

it("drops sidebar services onto Log workspace to open or focus tabs without mutations", async () => {
  render(<App />);
  const one = await screen.findByRole("button", { name: "One Running" });
  const two = screen.getByRole("button", { name: "Two Stopped" });
  const target = screen.getByRole("button", { name: "Log workspace" });
  const originalHitTest = Object.getOwnPropertyDescriptor(document, "elementFromPoint");
  Object.defineProperty(document, "elementFromPoint", {
    configurable: true,
    value: () => target.querySelector("svg"),
  });
  const start = (row: HTMLElement) => {
    fireEvent.pointerDown(row, { button: 0, pointerId: 1, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(row, { pointerId: 1, clientX: 50, clientY: 50 });
  };
  const drop = (row: HTMLElement) => {
    start(row);
    expect(target).toHaveClass("drop-active");
    fireEvent.pointerUp(row, { pointerId: 1, clientX: 50, clientY: 50 });
    fireEvent.click(row);
    expect(target).not.toHaveClass("drop-active");
  };
  start(one);
  fireEvent.pointerCancel(one, { pointerId: 1 });
  fireEvent.pointerUp(one, { pointerId: 1, clientX: 50, clientY: 50 });
  expect(target).not.toHaveClass("drop-active");
  expect(screen.queryByRole("tab")).not.toBeInTheDocument();
  drop(one);
  expect(await screen.findByRole("tab", { name: "One Running" })).toHaveAttribute("aria-selected", "true");
  drop(two);
  expect(screen.getByRole("tab", { name: "Two Stopped" })).toHaveAttribute("aria-selected", "true");
  drop(one);
  expect(screen.getAllByRole("tab")).toHaveLength(2);
  expect(screen.getByRole("tab", { name: "One Running" })).toHaveAttribute("aria-selected", "true");
  expect(api.moveService).not.toHaveBeenCalled();
  expect(api.control).not.toHaveBeenCalled();
  expect(api.controlSequence).not.toHaveBeenCalled();
  expect(workspace.groups[0].services).toHaveLength(2);
  if (originalHitTest) Object.defineProperty(document, "elementFromPoint", originalHitTest);
  else Reflect.deleteProperty(document, "elementFromPoint");
});
