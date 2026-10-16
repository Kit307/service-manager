import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import * as native from "../lib/api";
import { LogPanel } from "../components/LogPanel";
import type { Workspace } from "../lib/types";

function fixture(revision = "revision-a"): Workspace {
  return {
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
      await screen.findByRole("button", { name: "Fixture project" }),
    );
    await screen.findByText("retained-fixture-line");
    await user.click(screen.getByRole("button", { name: "Fixture project" }));
    expect(screen.getByText("retained-fixture-line")).toBeVisible();
  });
  it("keeps the original editor revision when background refresh observes an external edit", async () => {
    vi.spyOn(native, "isDesktop").mockReturnValue(true);
    const workspace = vi
      .spyOn(native.api, "workspace")
      .mockResolvedValue(fixture());
    vi.spyOn(native.api, "logs").mockResolvedValue({
      records: [],
      nextCursor: "0",
      truncated: false,
    });
    const save = vi
      .spyOn(native.api, "saveService")
      .mockRejectedValue({
        code: "CONFIG_CONFLICT",
        message: "External edit preserved. Reload before saving.",
      });
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Fixture project" }),
    );
    await user.click(screen.getByRole("button", { name: "Edit service" }));
    await user.clear(screen.getByLabelText("Service name"));
    await user.type(screen.getByLabelText("Service name"), "My edited name");
    workspace.mockResolvedValue(fixture("revision-b"));
    await new Promise((resolve) => setTimeout(resolve, 850));
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
  it("never offers removal of an existing service in the new-service form", async () => {
    vi.spyOn(native.api, "workspace").mockResolvedValue(fixture());
    const user = userEvent.setup();
    render(<App />);
    await user.click(
      await screen.findByRole("button", { name: "Add service" }),
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
});
