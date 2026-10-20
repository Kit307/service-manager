import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { AiConnections } from "../components/AiConnections";
import { api, pickProjectFolder } from "../lib/api";

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  return {
    ...original,
    pickProjectFolder: vi.fn(),
    api: { ...original.api, createClient: vi.fn() },
  };
});

it("bootstraps an AI client with a selected parent folder and no opened projects", async () => {
  vi.mocked(pickProjectFolder).mockResolvedValue("/workspace");
  vi.mocked(api.createClient).mockResolvedValue({
    id: "fixture",
    token: "fixture-token",
    proxyPath: "/fixture/proxy.sh",
  });
  const user = userEvent.setup();
  render(
    <AiConnections
      workspace={{
        projects: [],
        groups: [],
        ai: {
          enabled: true,
          endpoint: "http://127.0.0.1:47831/mcp",
          clients: [],
        },
        uncleanShutdown: false,
      }}
      onRefresh={async () => {}}
      onError={vi.fn()}
    />,
  );
  const create = screen.getByRole("button", { name: "Create profile & proxy" });
  expect(create).toBeEnabled();
  await user.click(screen.getByRole("button", { name: "Add allowed folder" }));
  expect(screen.getByText("/workspace")).toBeVisible();
  expect(
    screen.getByText(
      /Existing and future registered services are already accessible/,
    ),
  ).toBeVisible();
  expect(create).toBeEnabled();
  await user.click(
    screen.getByRole("button", { name: "Remove allowed folder /workspace" }),
  );
  expect(create).toBeEnabled();
  await user.click(screen.getByRole("button", { name: "Add allowed folder" }));
  await user.click(create);
  expect(api.createClient).toHaveBeenCalledWith("Codex", [], true, true, [
    "/workspace",
  ]);
  expect(await screen.findByLabelText("Proxy script")).toHaveValue(
    "/fixture/proxy.sh",
  );
});

it("creates a full-workspace token without choosing projects or roots", async () => {
  vi.mocked(api.createClient).mockResolvedValue({
    id: "fixture",
    token: "fixture-token",
    proxyPath: "/fixture/proxy.sh",
  });
  const user = userEvent.setup();
  render(
    <AiConnections
      workspace={{
        projects: [],
        groups: [],
        ai: { enabled: true, endpoint: "", clients: [] },
        uncleanShutdown: false,
      }}
      onRefresh={async () => {}}
      onError={vi.fn()}
    />,
  );
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  await user.click(
    screen.getByRole("button", { name: "Create profile & proxy" }),
  );
  expect(api.createClient).toHaveBeenLastCalledWith(
    "Codex",
    [],
    true,
    true,
    [],
  );
});
