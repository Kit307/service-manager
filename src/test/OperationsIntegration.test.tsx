import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
  expect(screen.getByRole("region", { name: "Service controls" })).toBeVisible();
  expect(screen.getByRole("group", { name: "Service command and status" })).toBeVisible();
  expect(screen.getByRole("tablist", { name: "Service views" })).toBeVisible();
  expect(screen.getAllByRole("tab").filter((tab) => tab.getAttribute("aria-selected") === "true")).toHaveLength(1);
  await userEvent.click(screen.getByRole("tab", { name: "History" }));
  expect(await screen.findByRole("region", { name: "Run history" })).toBeVisible();
  expect(screen.getByRole("heading", { level: 1, name: "One" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Restart" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Stop" })).toBeVisible();
  await userEvent.click(screen.getByRole("tab", { name: "Diagnostics" }));
  expect(await screen.findByRole("region", { name: "Service diagnostics" })).toBeVisible();
  expect(screen.getByRole("heading", { level: 1, name: "One" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Stop" })).toBeVisible();
  await userEvent.click(screen.getByRole("tab", { name: "Output" }));
  expect(screen.getByRole("region", { name: "Service logs" })).toBeVisible();
});
it("navigates service tabs by keyboard and associates each selected panel", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: /One Running/ }));
  const tablist = screen.getByRole("tablist", { name: "Service views" });
  const tabs = within(tablist).getAllByRole("tab");
  expect(tabs.map((tab) => tab.tabIndex)).toEqual([0, -1, -1, -1]);
  expect(new Set(tabs.map((tab) => tab.id)).size).toBe(4);
  expect(new Set(tabs.map((tab) => tab.getAttribute("aria-controls"))).size).toBe(4);
  expect(within(tablist).queryByRole("button", { name: "Open in log workspace" })).not.toBeInTheDocument();
  const check = (name: string) => {
    for (const item of tabs) {
      const panel = document.getElementById(item.getAttribute("aria-controls")!);
      expect(panel).toHaveAttribute("role", "tabpanel");
      expect(panel).toHaveAttribute("aria-labelledby", item.id);
    }
    const tab = within(tablist).getByRole("tab", { name });
    expect(tab).toHaveFocus();
    expect(tab).toHaveAttribute("aria-selected", "true");
    expect(tabs.filter((item) => item.tabIndex === 0)).toEqual([tab]);
    const panel = screen.getByRole("tabpanel", { name });
    expect(panel.id).toBe(tab.getAttribute("aria-controls"));
    expect(panel).toHaveAttribute("aria-labelledby", tab.id);
  };
  tabs[0].focus();
  check("Output");
  await user.keyboard("{ArrowRight}");
  check("Settings");
  await user.type(screen.getByLabelText("Service name"), " draft");
  within(tablist).getByRole("tab", { name: "Settings" }).focus();
  await user.keyboard("{ArrowRight}");
  check("History");
  await user.keyboard("{End}");
  check("Diagnostics");
  await user.keyboard("{ArrowRight}");
  check("Output");
  await user.keyboard("{ArrowLeft}");
  check("Diagnostics");
  await user.keyboard("{Home}");
  check("Output");
  await user.keyboard("{ArrowRight}");
  check("Settings");
  expect(screen.getByLabelText("Service name")).toHaveValue("One draft");
});

it.each(["click", "Enter", "Space"])("keeps focus on Settings after %s activation", async (activation) => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: /One Running/ }));
  const tab = screen.getByRole("tab", { name: "Settings" });
  if (activation === "click") await user.click(tab);
  else {
    tab.focus();
    await user.keyboard(activation === "Enter" ? "{Enter}" : " ");
  }
  expect(tab).toHaveFocus();
  expect(tab).toHaveAttribute("aria-selected", "true");
  expect(screen.getByLabelText("Service name")).toBeVisible();
});

it("opens the configured service URL through the native API", async () => {
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: /One Running/ }));
  await userEvent.click(screen.getByRole("button", { name: "Open URL" }));
  await waitFor(() => expect(api.openUrl).toHaveBeenCalledExactlyOnceWith("fixture", "one"));
});

it("runs, restarts, stops and opens logs for the selected service with shortcuts", async () => {
  vi.mocked(api.control).mockResolvedValue(undefined);
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: /Two Stopped/ }));
  fireEvent.keyDown(window, { key: "r", metaKey: true });
  await waitFor(() => expect(api.control).toHaveBeenCalledWith("fixture", "two", "start"));
  await userEvent.click(screen.getByRole("button", { name: /One Running/ }));
  fireEvent.keyDown(window, { key: "r", metaKey: true });
  await waitFor(() => expect(api.control).toHaveBeenCalledWith("fixture", "one", "restart"));
  fireEvent.keyDown(window, { key: ".", metaKey: true });
  await waitFor(() => expect(api.control).toHaveBeenCalledWith("fixture", "one", "stop"));
  fireEvent.keyDown(window, { key: "l", metaKey: true });
  expect(await screen.findByRole("tab", { name: "One Running" })).toBeVisible();
});

it("saves the settings form with Command-S without running while editing", async () => {
  vi.spyOn(api, "saveService").mockResolvedValue(workspace.projects[0]);
  render(<App />);
  await userEvent.click(await screen.findByRole("button", { name: /One Running/ }));
  await userEvent.click(screen.getByRole("tab", { name: "Settings" }));
  const input = screen.getByLabelText("Service name");
  input.focus();
  fireEvent.keyDown(input, { key: "r", metaKey: true });
  expect(api.control).not.toHaveBeenCalled();
  fireEvent.keyDown(input, { key: "s", metaKey: true });
  await waitFor(() => expect(api.saveService).toHaveBeenCalledTimes(1));
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
