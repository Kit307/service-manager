import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import App from "../App";
import { GroupOverview } from "../components/GroupOverview";
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
      controlSequence: vi.fn(),
      cancelSequence: vi.fn(),
      trustService: vi.fn(),
    },
  };
});
let project: Project;
let outside: Project;
beforeEach(() => {
  vi.resetAllMocks();
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
  vi.mocked(api.controlSequence).mockResolvedValue(undefined);
  vi.mocked(api.cancelSequence).mockResolvedValue(undefined);
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
it("Run all sends the selected group including active readiness dependencies", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Run all" }));
  await waitFor(() =>
    expect(api.controlSequence).toHaveBeenCalledWith([
      { projectId: "fixture", serviceId: "one" },
      { projectId: "fixture", serviceId: "two" },
    ], "start"),
  );
  expect(api.controlSequence).toHaveBeenCalledTimes(1);
  expect(api.control).not.toHaveBeenCalled();
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
  expect(api.controlSequence).not.toHaveBeenCalled();
  expect(api.trustService).not.toHaveBeenCalled();
});
it("reports fail-fast sequence errors without falling back to individual controls", async () => {
  vi.mocked(api.controlSequence).mockRejectedValue(new Error("Fixture readiness failed; sequence stopped"));
  const user = userEvent.setup();
  render(<App />);
  await user.click(await screen.findByRole("button", { name: "Example group" }));
  await user.click(screen.getByRole("button", { name: "Restart all" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Fixture readiness failed; sequence stopped");
  expect(api.controlSequence).toHaveBeenCalledExactlyOnceWith([
    { projectId: "fixture", serviceId: "one" },
    { projectId: "fixture", serviceId: "two" },
  ], "restart");
  expect(api.control).not.toHaveBeenCalled();
});

it("Stop all touches only active services in the selected group", async () => {
  const user = userEvent.setup();
  render(<App />);
  await user.click(
    await screen.findByRole("button", { name: "Example group" }),
  );
  await user.click(screen.getByRole("button", { name: "Stop all" }));
  await waitFor(() =>
    expect(api.controlSequence).toHaveBeenCalledWith([
      { projectId: "fixture", serviceId: "one" },
    ], "stop"),
  );
  expect(api.controlSequence).toHaveBeenCalledTimes(1);
  expect(api.control).not.toHaveBeenCalled();
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
  expect(api.controlSequence).not.toHaveBeenCalled();
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
  expect(api.controlSequence).not.toHaveBeenCalled();
});

it("keeps compact card metadata discoverable and opens the selected service logs", async () => {
  project.runtime.two = {
    state: "failed",
    health: "notConfigured",
    error: "Port 4200 is already in use.",
  };
  const services = project.config.services.map((service) => ({ project, service }));
  const onService = vi.fn();
  render(<GroupOverview name="Example group" services={services} busy={false} results={[]}
    onControl={vi.fn()} onService={onService} onAdd={vi.fn()} />);
  expect(screen.getAllByRole("article")).toHaveLength(2);
  expect(screen.getAllByTitle("sleep 60")).toHaveLength(2);
  expect(screen.getAllByTitle("/fixture/.")).toHaveLength(2);
  expect(screen.getByRole("status")).toHaveTextContent("Port 4200 is already in use.");
  await userEvent.setup().click(screen.getByRole("button", { name: "View logs for Two" }));
  expect(onService).toHaveBeenCalledExactlyOnceWith(services[1]);
});
