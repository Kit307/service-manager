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
  expect(row).toHaveAttribute("draggable", "true");
  const payload: Record<string, string> = {};
  const dataTransfer = {
    setData: (type: string, value: string) => {
      payload[type] = value;
    },
    getData: (type: string) => payload[type] ?? "",
    types: ["application/x-service-manager-service"],
    effectAllowed: "",
    dropEffect: "",
  };
  fireEvent.dragStart(row, { dataTransfer });
  fireEvent.dragOver(screen.getByLabelText("Move service to Ungrouped"), {
    dataTransfer,
  });
  fireEvent.drop(screen.getByLabelText("Move service to Ungrouped"), {
    dataTransfer,
  });
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
  fireEvent.dragStart(screen.getByRole("button", { name: "Web Stopped" }), {
    dataTransfer,
  });
  fireEvent.drop(screen.getByLabelText("Move service to group Demo"), {
    dataTransfer,
  });
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
it("rejects foreign drag payloads and preserves membership when saving fails", async () => {
  render(<App />);
  const row = await screen.findByRole("button", { name: "Web Stopped" });
  const target = screen.getByLabelText("Move service to Ungrouped");
  fireEvent.drop(target, {
    dataTransfer: { getData: () => "not-json", types: ["text/plain"] },
  });
  fireEvent.drop(target, {
    dataTransfer: {
      getData: () => JSON.stringify({ projectId: "foreign", serviceId: "web" }),
      types: ["application/x-service-manager-service"],
    },
  });
  expect(api.moveService).not.toHaveBeenCalled();
  vi.mocked(api.moveService).mockRejectedValue({
    message: "Could not move service",
  });
  fireEvent.drop(target, {
    dataTransfer: {
      getData: () => JSON.stringify({ projectId: "p", serviceId: "web" }),
      types: ["application/x-service-manager-service"],
    },
  });
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Could not move service",
  );
  expect(row.closest(".workspace-group")).not.toBeNull();
});
