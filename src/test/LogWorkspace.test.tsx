import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { LogWorkspace } from "../components/LogWorkspace";
import { api, isDesktop } from "../lib/api";
import { refKey, type ServiceEntry } from "../lib/workspace";

vi.mock("../lib/api", async (original) => ({ ...await original<typeof import("../lib/api")>(), isDesktop: vi.fn() }));
const service = (id: string): ServiceEntry => ({
  project: { id: "fixture", name: "Fixture", path: "/fixture", revision: "r", trusted: [], runtime: {}, config: { schemaVersion: 1, services: [] } },
  service: { id, name: id, type: "command", command: "fixture", cwd: "." },
});
const services = [service("API"), service("Web"), service("Worker")];
const ref = (serviceId: string) => ({ projectId: "fixture", serviceId });
beforeEach(() => {
  localStorage.clear();
  vi.mocked(isDesktop).mockReturnValue(true);
  vi.spyOn(api, "logs").mockImplementation(async (_p, id, cursor) => ({ records: cursor ? [] : [{ seq: 1, runId: "run", timestamp: "2026-01-01T00:00:00Z", stream: "stdout", text: `${id} output`, truncated: false }], nextCursor: "1", truncated: false }));
  vi.spyOn(api, "onLogs").mockResolvedValue(() => {});
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it("opens distinct tabs, retains independent search, and closes observation without stopping services", async () => {
  const user = userEvent.setup();
  const control = vi.spyOn(api, "control");
  const clear = vi.spyOn(api, "clearLogs");
  const { rerender } = render(<LogWorkspace services={services} requested={ref("API")} />);
  await screen.findByText("API output");
  await user.type(screen.getByRole("textbox", { name: "Search logs" }), "API");
  rerender(<LogWorkspace services={services} requested={ref("Web")} />);
  await screen.findByText("Web output");
  expect(screen.getByRole("textbox", { name: "Search logs" })).toHaveValue("");
  await user.click(screen.getByRole("tab", { name: "API Stopped" }));
  expect(screen.getByRole("textbox", { name: "Search logs" })).toHaveValue("API");
  await user.click(screen.getByRole("button", { name: "Close API log tab" }));
  expect(screen.queryByRole("tab", { name: "API Stopped" })).not.toBeInTheDocument();
  expect(control).not.toHaveBeenCalled();
  expect(clear).not.toHaveBeenCalled();
});

it("moves tabs through keyboard controls and creates more than two panes", async () => {
  const user = userEvent.setup();
  const { rerender } = render(<LogWorkspace services={services} requested={ref("API")} />);
  rerender(<LogWorkspace services={services} requested={ref("Web")} />);
  await user.click(screen.getByRole("button", { name: "Move active log tab left" }));
  expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Web", "API"]);
  await user.selectOptions(screen.getByRole("combobox", { name: "Move active log tab to pane" }), "new");
  await user.click(screen.getByRole("button", { name: "Add pane" }));
  await user.click(screen.getByRole("button", { name: "Add pane" }));
  expect(screen.getAllByRole("region", { name: "Log pane" })).toHaveLength(4);
  const panes = screen.getAllByRole("region", { name: "Log pane" });
  expect(within(panes[0]).getByRole("tab", { name: "API Stopped" })).toBeVisible();
  expect(within(panes[1]).getByRole("tab", { name: "Web Stopped" })).toBeVisible();
});

it("keeps toolbar moves on the tab selected by arrow keys in a later pane", async () => {
  localStorage.setItem("log-workspace-v1", JSON.stringify([
    { id: "first", tabs: [ref("API")], active: refKey(ref("API")) },
    { id: "second", tabs: [ref("Web"), ref("Worker")], active: refKey(ref("Web")) },
  ]));
  const user = userEvent.setup();
  render(<LogWorkspace services={services} />);
  const web = screen.getByRole("tab", { name: "Web Stopped" });
  act(() => web.focus());
  await user.keyboard("{ArrowRight}");
  expect(screen.getByRole("tab", { name: "Worker Stopped" })).toHaveFocus();
  await user.selectOptions(screen.getByRole("combobox", { name: "Move active log tab to pane" }), "new");
  const panes = screen.getAllByRole("region", { name: "Log pane" });
  expect(panes).toHaveLength(3);
  expect(within(panes[0]).getByRole("tab", { name: "API Stopped" })).toBeVisible();
  expect(within(panes[2]).getByRole("tab", { name: "Worker Stopped" })).toBeVisible();
});

it("keeps unlimited restored metadata and unavailable references through initial workspace loading", async () => {
  const many = Array.from({ length: 12 }, (_, index) => service(`Service ${index}`));
  const tabs = many.map((item) => ref(item.service.id));
  localStorage.setItem("log-workspace-v1", JSON.stringify([{ id: "restored", tabs, active: refKey(tabs[0]) }]));
  const { rerender } = render(<LogWorkspace services={[]} />);
  expect(screen.getAllByRole("tab", { name: "Unavailable service" })).toHaveLength(12);
  rerender(<LogWorkspace services={many} />);
  expect(screen.getAllByRole("tab")).toHaveLength(12);
  expect(screen.getByRole("tab", { name: "Service 0 Stopped" })).toHaveAttribute("aria-selected", "true");
  expect(api.logs).toHaveBeenCalledWith("fixture", "Service 0", undefined);
});

it("observes only in-viewport active panes and pauses when workspace is hidden", async () => {
  const observers: Array<(entries: { isIntersecting: boolean }[]) => void> = [];
  vi.stubGlobal("IntersectionObserver", class {
    constructor(callback: (entries: { isIntersecting: boolean }[]) => void) { observers.push(callback); }
    observe() {} disconnect() {}
  });
  const { rerender } = render(<LogWorkspace services={services} requested={ref("API")} />);
  expect(api.logs).not.toHaveBeenCalled();
  await act(async () => observers.at(-1)?.([{ isIntersecting: true }]));
  await screen.findByText("API output");
  vi.mocked(api.logs).mockClear();
  rerender(<LogWorkspace services={services} requested={ref("API")} visible={false} />);
  expect(screen.queryByText("API output")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Log workspace")).not.toBeVisible();
  await waitFor(() => expect(api.logs).not.toHaveBeenCalled());
  vi.unstubAllGlobals();
});

it("preserves unrelated empty panes and targets the requested or clicked pane for moves", async () => {
  const user = userEvent.setup();
  const { rerender } = render(<LogWorkspace services={services} requested={ref("API")} />);
  await user.click(screen.getByRole("button", { name: "Add pane" }));
  await user.click(screen.getByRole("button", { name: "Add pane" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "Move active log tab to pane" }), "new");
  rerender(<LogWorkspace services={services} requested={ref("Web")} />);
  await user.selectOptions(screen.getByRole("combobox", { name: "Move active log tab to pane" }), "new");
  expect(screen.getAllByRole("region", { name: "Log pane" })).toHaveLength(3);
  await user.click(screen.getByRole("button", { name: "Close Web log tab" }));
  expect(screen.getAllByRole("region", { name: "Log pane" })).toHaveLength(2);
});

it("uses captured pointer drops to reorder and creates panes only for explicit internal targets", async () => {
  const { rerender } = render(<LogWorkspace services={services} requested={ref("API")} />);
  rerender(<LogWorkspace services={services} requested={ref("Web")} />);
  const web = screen.getByRole("tab", { name: "Web Stopped" });
  const apiTab = screen.getByRole("tab", { name: "API Stopped" });
  const hitTest = vi.fn(() => apiTab);
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: hitTest });
  fireEvent.pointerDown(web, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(web, { clientX: 50, clientY: 50 });
  fireEvent.pointerUp(web, { clientX: 50, clientY: 50 });
  expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Web", "API"]);
  fireEvent.pointerDown(web, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(web, { clientX: 50, clientY: 50 });
  hitTest.mockReturnValue(document.body);
  fireEvent.pointerMove(web, { clientX: 60, clientY: 60 });
  expect(screen.getByLabelText("Log workspace")).toHaveClass("dragging-log");
  expect(screen.getByText("Drop a tab here to create a pane")).toBeVisible();
  hitTest.mockReturnValue(screen.getByText("Drop a tab here to create a pane"));
  fireEvent.pointerUp(web, { clientX: 50, clientY: 50 });
  expect(screen.getAllByRole("region", { name: "Log pane" })).toHaveLength(2);
  fireEvent.pointerDown(apiTab, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(apiTab, { clientX: 50, clientY: 50 });
  fireEvent.keyDown(apiTab, { key: "Escape" });
  fireEvent.pointerUp(apiTab, { clientX: 50, clientY: 50 });
  expect(screen.getAllByRole("region", { name: "Log pane" })).toHaveLength(2);
});
