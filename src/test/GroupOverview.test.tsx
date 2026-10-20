import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import App from "../App";
import { api, confirmAction } from "../lib/api";
import type { Project, Workspace } from "../lib/types";

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  return {
    ...original,
    confirmAction: vi.fn(),
    api: {
      ...original.api,
      workspace: vi.fn(),
      control: vi.fn(),
      trustService: vi.fn(),
    },
  };
});
let project: Project;
let outside: Project;
beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  project = {
    id: "fixture",
    name: "Example",
    path: "/fixture",
    revision: "rev",
    config: {
      schemaVersion: 1,
      services: [
        {
          id: "one",
          name: "One",
          cwd: ".",
          type: "command",
          command: "sleep 60",
        },
        {
          id: "two",
          name: "Two",
          cwd: ".",
          type: "command",
          command: "sleep 60",
        },
      ],
    },
    trusted: ["one", "two"],
    runtime: { one: { state: "running", health: "notConfigured" } },
  };
  outside = { ...project, id: "outside", name: "Other folder", path: "/other" };
  vi.mocked(api.workspace).mockImplementation(
    async () =>
      ({
        projects: [project, outside],
        groups: [
          {
            id: "g",
            name: "Example group",
            services: [
              { projectId: "fixture", serviceId: "one" },
              { projectId: "fixture", serviceId: "two" },
            ],
          },
        ],
        ai: { enabled: false, endpoint: "", clients: [] },
        uncleanShutdown: false,
      }) as Workspace,
  );
  vi.mocked(confirmAction).mockResolvedValue(true);
  vi.mocked(api.control).mockResolvedValue(undefined);
});
it("collapses a group independently from selection and remembers the choice", async () => {
  const user = userEvent.setup();
  const first = render(<App />);
  const toggle = await screen.findByRole("button", {
    name: "Collapse group Example group",
  });
  await user.click(toggle);
  expect(
    screen.getAllByRole("button", { name: /One Running/ }),
  ).toHaveLength(1);
  expect(
    screen.getByRole("button", { name: "Expand group Example group" }),
  ).toHaveAttribute("aria-expanded", "false");
  first.unmount();
  render(<App />);
  expect(
    await screen.findByRole("button", { name: "Expand group Example group" }),
  ).toHaveAttribute("aria-expanded", "false");
});
it("opens group overview and Run all skips active services", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Run all" }));
  await waitFor(() =>
    expect(api.control).toHaveBeenCalledWith("fixture", "two", "start"),
  );
  expect(api.control).toHaveBeenCalledTimes(1);
});
it("does not start anything if the group trust review is canceled", async () => {
  project.trusted = [];
  vi.mocked(confirmAction).mockResolvedValue(false);
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Run all" }));
  expect(confirmAction).toHaveBeenCalled();
  expect(api.control).not.toHaveBeenCalled();
  expect(api.trustService).not.toHaveBeenCalled();
});
it("reports per-service restart errors while still requesting other services", async () => {
  vi.mocked(api.control).mockImplementation(async (_p, id) => {
    if (id === "one") throw { message: "Fixture restart failed" };
  });
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Restart all" }));
  expect(await screen.findByText("Fixture restart failed")).toBeVisible();
  expect(api.control).toHaveBeenCalledWith("fixture", "two", "restart");
});

it("Stop all touches only active services in the selected group", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Stop all" }));
  await waitFor(() =>
    expect(api.control).toHaveBeenCalledWith("fixture", "one", "stop"),
  );
  expect(api.control).toHaveBeenCalledTimes(1);
  expect(confirmAction).not.toHaveBeenCalled();
});
it("completes every trust review before trusting or running a batch", async () => {
  project.trusted = [];
  vi.mocked(confirmAction)
    .mockResolvedValueOnce(true)
    .mockResolvedValueOnce(false);
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Restart all" }));
  await waitFor(() => expect(confirmAction).toHaveBeenCalledTimes(2));
  expect(api.trustService).not.toHaveBeenCalled();
  expect(api.control).not.toHaveBeenCalled();
});
it("aborts lifecycle requests when a reviewed revision becomes stale", async () => {
  project.trusted = [];
  vi.mocked(api.trustService).mockRejectedValue({
    message: "Configuration changed while reviewing",
  });
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Restart all" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Configuration changed while reviewing",
  );
  expect(api.trustService).toHaveBeenCalledWith("fixture", "one", "rev");
  expect(api.control).not.toHaveBeenCalled();
});
