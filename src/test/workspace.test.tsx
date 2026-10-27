import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import * as native from "../lib/api";
import { LogPanel } from "../components/LogPanel";
import { AiConnections } from "../components/AiConnections";
import type { Workspace } from "../lib/types";

function fixture(revision = "revision-a"): Workspace {
  return {
    groups: [],
    projects: [
      {
        id: "p1",
        name: "Fixture project",
        path: "/isolated-fixture",
        revision,
        config: {
          schemaVersion: 1,
          services: [
            {
              id: "web",
              name: "Frontend",
              type: "command",
              cwd: ".",
              command: "pnpm dev",
            },
          ],
        },
        trusted: [],
        runtime: {},
      },
    ],
    ai: { enabled: false, endpoint: "http://127.0.0.1:47831/mcp", clients: [] },
    uncleanShutdown: false,
  };
}
afterEach(() => vi.restoreAllMocks());
describe("workspace interactions", () => {
  it("keeps retained output when selecting the same project and service again", async () => {
    vi.spyOn(native, "isDesktop").mockReturnValue(true);
    vi.spyOn(native.api, "workspace").mockResolvedValue(fixture());
    vi.spyOn(native.api, "logs").mockImplementation(
      async (_project, _service, cursor) => ({
        records: cursor
          ? []
          : [
              {
                seq: 1,
                runId: "r1",
                timestamp: "2026-10-16T00:00:00Z",
                stream: "stdout",
                text: "retained-fixture-line",
                truncated: false,
              },
            ],
        nextCursor: "1",
        truncated: false,
      }),
    );
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Frontend Stopped" }),
    );
    await screen.findByText("retained-fixture-line");
    await user.click(screen.getByRole("button", { name: "Frontend Stopped" }));
    expect(screen.getByText("retained-fixture-line")).toBeVisible();
  });
  it("keeps the original editor revision when background refresh observes an external edit", async () => {
    vi.spyOn(native, "isDesktop").mockReturnValue(true);
    vi.spyOn(native.api, "workspace").mockResolvedValue(fixture());
    let onWorkspace: ((data: Workspace) => void) | undefined;
    vi.spyOn(native.api, "onWorkspace").mockImplementation(async (callback) => {
      onWorkspace = callback;
      return () => {};
    });
    vi.spyOn(native.api, "logs").mockResolvedValue({
      records: [],
      nextCursor: "0",
      truncated: false,
    });
    const save = vi.spyOn(native.api, "saveService").mockRejectedValue({
      code: "CONFIG_CONFLICT",
      message: "External edit preserved. Reload before saving.",
    });
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Frontend Stopped" }),
    );
    await user.click(screen.getByRole("button", { name: "Edit service" }));
    await user.clear(screen.getByLabelText("Service name"));
    await user.type(screen.getByLabelText("Service name"), "My edited name");
    await act(async () => onWorkspace?.(fixture("revision-b")));
    await user.click(screen.getByRole("tab", { name: "Output" }));
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    expect(screen.getByLabelText("Service name")).toHaveValue("My edited name");
    await user.click(screen.getByRole("button", { name: "Save service" }));
    await waitFor(() =>
      expect(screen.getByRole("alert")).toHaveTextContent(
        "External edit preserved",
      ),
    );
    expect(save).toHaveBeenCalledWith(
      "p1",
      expect.objectContaining({ name: "My edited name" }),
      "revision-a",
    );
    expect(screen.getByLabelText("Service name")).toHaveValue("My edited name");
  });
  it("keeps the service heading and controls while Settings uses the content pane", async () => {
    const data = fixture();
    data.projects[0].runtime.web = {
      state: "running",
      health: "notConfigured",
    };
    vi.spyOn(native.api, "workspace").mockResolvedValue(data);
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Frontend Running" }),
    );
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Frontend" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Stop" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Restart" })).toBeVisible();
    expect(screen.getByRole("tab", { name: "Settings" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    expect(screen.getByLabelText("Service name")).toBeVisible();
    await user.type(screen.getByLabelText("Service name"), " draft");
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("region", { name: "Log output" })).toBeVisible();
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    expect(screen.getByLabelText("Service name")).toHaveValue("Frontend");
  });
  it("retains log search and records when switching between Output and Settings", async () => {
    vi.spyOn(native, "isDesktop").mockReturnValue(true);
    vi.spyOn(native.api, "workspace").mockResolvedValue(fixture());
    vi.spyOn(native.api, "logs").mockImplementation(async (_p, _s, cursor) => ({
      records: cursor
        ? []
        : [
            {
              seq: 1,
              runId: "r",
              timestamp: "2026-10-20T00:00:00Z",
              stream: "stderr",
              text: "INF Success status=200",
              truncated: false,
            },
          ],
      nextCursor: "1",
      truncated: false,
    }));
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Frontend Stopped" }),
    );
    await screen.findByText("INF Success status=200");
    await user.type(
      screen.getByRole("textbox", { name: "Search logs" }),
      "Success",
    );
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    await user.click(screen.getByRole("tab", { name: "Output" }));
    expect(screen.getByRole("textbox", { name: "Search logs" })).toHaveValue(
      "Success",
    );
    expect(screen.getByText("INF Success status=200")).toBeVisible();
  });
  it("never offers removal of an existing service in the new-service form", async () => {
    vi.spyOn(native.api, "workspace").mockResolvedValue(fixture());
    vi.spyOn(native, "pickProjectFolder").mockResolvedValue(
      "/isolated-fixture",
    );
    vi.spyOn(native.api, "openProject").mockResolvedValue(
      fixture().projects[0],
    );
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      (await screen.findAllByRole("button", { name: "Add service" }))[0],
    );
    expect(
      screen.getByRole("heading", { name: "Add a service" }),
    ).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Remove service" }),
    ).not.toBeInTheDocument();
  });
  it("filters actual output records and renders markup as inert text", async () => {
    const user = userEvent.setup();
    render(
      <LogPanel
        logs={[
          {
            seq: 1,
            runId: "r1",
            timestamp: "2026-10-16T00:00:00Z",
            stream: "stdout",
            text: "<script>untrusted()</script>",
            truncated: false,
          },
          {
            seq: 2,
            runId: "r1",
            timestamp: "2026-10-16T00:00:01Z",
            stream: "stderr",
            text: "fixture failure",
            truncated: false,
          },
        ]}
        active={true}
        gap={false}
        onClear={() => {}}
      />,
    );
    expect(screen.getByText("<script>untrusted()</script>")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "stderr" }));
    expect(screen.getByText("fixture failure")).toBeVisible();
    expect(
      screen.queryByText("<script>untrusted()</script>"),
    ).not.toBeInTheDocument();
  });
  it("reveals private Codex and Claude configuration only for the new profile", async () => {
    const user = userEvent.setup();
    const data = fixture();
    data.ai.enabled = true;
    vi.spyOn(native.api, "createClient").mockResolvedValue({
      id: "client-fixture",
      token: "synthetic-token",
      proxyPath: "/private-fixture/proxy.sh",
    });
    render(
      <AiConnections
        workspace={data}
        onRefresh={async () => {}}
        onError={() => {}}
      />,
    );
    expect(
      screen.queryByLabelText("Claude Code MCP configuration"),
    ).not.toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Create profile & proxy" }),
    );
    const config = await screen.findByLabelText(
      "Claude Code MCP configuration",
    );
    expect(
      JSON.parse((config as HTMLTextAreaElement).value).mcpServers
        .service_manager.headers.Authorization,
    ).toBe("Bearer synthetic-token");
    expect(
      (screen.getByLabelText("Codex MCP configuration") as HTMLTextAreaElement)
        .value,
    ).toContain("[mcp_servers.service_manager]");
    await user.click(screen.getByRole("button", { name: "Hide token" }));
    expect(
      screen.queryByLabelText("Claude Code MCP configuration"),
    ).not.toBeInTheDocument();
  });
});
