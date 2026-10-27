import {
  render,
  screen,
  waitFor,
  within,
  fireEvent,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import App from "../App";
import { api, confirmAction } from "../lib/api";
import type { Workspace } from "../lib/types";
vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  return {
    ...original,
    confirmAction: vi.fn(),
    api: {
      ...original.api,
      workspace: vi.fn(),
      saveGroup: vi.fn(),
      removeGroup: vi.fn(),
      moveService: vi.fn(),
    },
  };
});
let data: Workspace;
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  data = {
    projects: [
      {
        id: "p",
        name: "Hidden root",
        path: "/fixture",
        revision: "rev",
        trusted: [],
        runtime: {},
        config: {
          schemaVersion: 1,
          services: [
            {
              id: "web",
              name: "Web",
              cwd: ".",
              type: "command",
              command: "pnpm dev",
            },
          ],
        },
      },
    ],
    groups: [
      {
        id: "g",
        name: "Demo",
        services: [{ projectId: "p", serviceId: "web" }],
      },
    ],
    ai: { enabled: false, endpoint: "", clients: [] },
    uncleanShutdown: false,
  };
  vi.mocked(api.workspace).mockImplementation(async () => data);
  vi.mocked(confirmAction).mockResolvedValue(true);
  vi.mocked(api.removeGroup).mockImplementation(async () => {
    data = { ...data, groups: [] };
  });
  vi.mocked(api.saveGroup).mockImplementation(async (id, name, services) => {
    const group = { id: id ?? "new", name, services };
    data = { ...data, groups: [group] };
    return group;
  });
});
it("shows existing services directly without a project row", async () => {
  render(<App />);
  expect(
    await screen.findByRole("button", { name: "Web Stopped" }),
  ).toBeVisible();
  expect(
    screen.queryByRole("button", { name: "Hidden root" }),
  ).not.toBeInTheDocument();
  expect(screen.getByText("pnpm dev")).toBeVisible();
  expect(
    screen
      .getByRole("button", { name: "Edit group Demo" })
      .querySelector("svg"),
  ).toHaveAttribute("data-icon", "settings");
  expect(
    screen.getByRole("button", { name: /All services/ }).querySelector("svg"),
  ).toHaveAttribute("data-icon", "grid");
  expect(document.querySelector(".open-folder svg")).toHaveAttribute(
    "data-icon",
    "plus",
  );
});
it("shows the service state as text in its accessible row", async () => {
  data.projects[0].config.services[0].name = "Merchant Portal";
  data.projects[0].runtime.web = { state: "ready", health: "notConfigured" };
  render(<App />);
  const row = await screen.findByRole("button", { name: /Merchant Portal.*Ready/i });
  expect(within(row).getByText("Ready")).toBeVisible();
});
it("removes only the group and retains its services", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Edit group Demo" }),
  );
  await user.click(screen.getByRole("button", { name: "Remove group" }));
  await waitFor(() => expect(api.removeGroup).toHaveBeenCalledWith("g"));
  expect(confirmAction).toHaveBeenCalledWith(
    expect.stringContaining("services will remain available"),
    "Remove group?",
  );
  expect(
    await screen.findByRole("button", { name: "Web Stopped" }),
  ).toBeVisible();
});
it("creates a group with existing service refs", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "New group" }));
  await user.type(screen.getByLabelText("Group name"), "New group");
  await user.click(screen.getByRole("checkbox", { name: /Web/ }));
  await user.click(screen.getByRole("button", { name: "Create group" }));
  await waitFor(() =>
    expect(api.saveGroup).toHaveBeenCalledWith(undefined, "New group", [
      { projectId: "p", serviceId: "web" },
    ]),
  );
});
it("supports changing group membership with the keyboard", async () => {
  const user = userEvent.setup();
  render(<App />);
  const edit = await screen.findByRole("button", { name: "Edit group Demo" });
  edit.focus();
  await user.keyboard("{Enter}");
  const member = screen.getByRole("checkbox", { name: /Web/ });
  member.focus();
  await user.keyboard("[Space]");
  const save = screen.getByRole("button", { name: "Save group" });
  save.focus();
  await user.keyboard("{Enter}");
  await waitFor(() => expect(api.saveGroup).toHaveBeenCalledWith("g", "Demo", []));
});
it("preserves the editor and services when group persistence fails", async () => {
  const user = userEvent.setup();
  vi.mocked(api.saveGroup).mockRejectedValue({
    message: "Could not save group",
  });
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Edit group Demo" }),
  );
  await user.click(screen.getByRole("button", { name: "Save group" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Could not save group",
  );
  expect(screen.getByLabelText("Group name")).toHaveValue("Demo");
  expect(
    within(
      screen.getByRole("navigation", { name: "Groups and services" }),
    ).getByRole("button", { name: "Web Stopped" }),
  ).toBeVisible();
});

it("drags an existing service to Ungrouped then into a group without editing definitions", async () => {
  vi.mocked(api.moveService).mockImplementation(async (ref, target) => {
    data = {
      ...data,
      groups: data.groups.map((g) => ({
        ...g,
        services: [
          ...g.services.filter(
            (s) =>
              s.projectId !== ref.projectId || s.serviceId !== ref.serviceId,
          ),
          ...(g.id === target ? [ref] : []),
        ],
      })),
    };
  });
  render(<App />);
  const row = await screen.findByRole("button", { name: "Web Stopped" });
  pointerDrag(row, screen.getByLabelText("Move service to Ungrouped"));
  await waitFor(() =>
    expect(api.moveService).toHaveBeenCalledWith(
      { projectId: "p", serviceId: "web" },
      undefined,
    ),
  );
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Web Stopped" })
        .closest(".workspace-group"),
    ).toBeNull(),
  );
  pointerDrag(
    screen.getByRole("button", { name: "Web Stopped" }),
    screen.getByLabelText("Move service to group Demo"),
  );
  await waitFor(() =>
    expect(api.moveService).toHaveBeenLastCalledWith(
      { projectId: "p", serviceId: "web" },
      "g",
    ),
  );
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "Web Stopped" })
        .closest(".workspace-group"),
    ).not.toBeNull(),
  );
});
it("ignores outside drops and cancelled drags; preserves membership when saving fails", async () => {
  render(<App />);
  const row = await screen.findByRole("button", { name: "Web Stopped" });
  const target = screen.getByLabelText("Move service to Ungrouped");
  pointerDrag(row, document.body);
  fireEvent.pointerDown(row, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(row, { clientX: 50, clientY: 50 });
  fireEvent.pointerCancel(row);
  fireEvent.pointerUp(row, { clientX: 50, clientY: 50 });
  expect(api.moveService).not.toHaveBeenCalled();
  vi.mocked(api.moveService).mockRejectedValue({
    message: "Could not move service",
  });
  pointerDrag(row, target);
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Could not move service",
  );
  expect(row.closest(".workspace-group")).not.toBeNull();
});

it("keeps clicks as selection and cancels a drag with Escape", async () => {
  render(<App />);
  const row = await screen.findByRole("button", { name: "Web Stopped" });
  Object.defineProperty(document, "elementFromPoint", {
    configurable: true,
    value: () => screen.getByLabelText("Move service to Ungrouped"),
  });
  fireEvent.pointerDown(row, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(row, { clientX: 11, clientY: 11 });
  fireEvent.pointerUp(row, { clientX: 11, clientY: 11 });
  expect(api.moveService).not.toHaveBeenCalled();
  fireEvent.click(row);
  expect(screen.getByRole("button", { name: "Output" })).toBeVisible();
  fireEvent.pointerDown(row, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(row, { clientX: 50, clientY: 50 });
  fireEvent.keyDown(row, { key: "Escape" });
  fireEvent.pointerUp(row, { clientX: 50, clientY: 50 });
  expect(api.moveService).not.toHaveBeenCalled();
});

function pointerDrag(row: HTMLElement, target: HTMLElement) {
  Object.defineProperty(document, "elementFromPoint", {
    configurable: true,
    value: () => target,
  });
  fireEvent.pointerDown(row, { button: 0, clientX: 10, clientY: 10 });
  fireEvent.pointerMove(row, { clientX: 50, clientY: 50 });
  fireEvent.pointerUp(row, { clientX: 50, clientY: 50 });
}
