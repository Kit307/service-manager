import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { api, confirmAction } from "../lib/api";
import type { Project, Workspace } from "../lib/types";

vi.mock("../lib/api", async (importOriginal) => {
  const original = await importOriginal<typeof import("../lib/api")>();
  return { ...original, confirmAction: vi.fn(), api: { ...original.api, workspace: vi.fn(), removeProject: vi.fn() } };
});

let projects: Project[];
const workspace = (): Workspace => ({ projects, ai: { enabled: false, endpoint: "", clients: [] }, uncleanShutdown: false });
beforeEach(() => {
  vi.clearAllMocks();
  projects = [{ id: "one", name: "Example", path: "/example", revision: "absent", config: { schemaVersion: 1, services: [] }, trusted: [], runtime: {} }];
  vi.mocked(api.workspace).mockImplementation(async () => workspace());
  vi.mocked(confirmAction).mockResolvedValue(true);
  vi.mocked(api.removeProject).mockImplementation(async (id) => { projects = projects.filter((p) => p.id !== id); });
});

describe("remove project", () => {
  it("confirms preservation of files and returns to the empty workspace", async () => {
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Example" }));
    await user.click(screen.getByRole("button", { name: "Remove project Example" }));
    await waitFor(() => expect(api.removeProject).toHaveBeenCalledWith("one"));
    expect(confirmAction).toHaveBeenCalledWith(expect.stringContaining("code folder and saved service configuration will be kept"), "Remove project?");
    await waitFor(() => expect(screen.queryByRole("button", { name: "Remove project Example" })).not.toBeInTheDocument());
    expect(screen.getByRole("heading", { name: /Your services/ })).toBeVisible();
  });

  it("keeps the project when confirmation is canceled or persistence fails", async () => {
    const user = userEvent.setup();
    vi.mocked(confirmAction).mockResolvedValueOnce(false);
    render(<App />);
    const remove = await screen.findByRole("button", { name: "Remove project Example" });
    await user.click(remove);
    expect(api.removeProject).not.toHaveBeenCalled();
    vi.mocked(api.removeProject).mockRejectedValueOnce({ message: "Could not save settings" });
    await user.click(remove);
    expect(await screen.findByText("Could not save settings")).toBeVisible();
    expect(remove).toBeInTheDocument();
  });

  it("requires active services to stop before removal", async () => {
    projects[0].runtime.web = { state: "stopping", health: "notConfigured" };
    render(<App />);
    const remove = await screen.findByRole("button", { name: "Remove project Example" });
    expect(remove).toBeDisabled();
    expect(remove).toHaveAttribute("title", "Stop all services before removing this project");
  });

  it("explains preserved Docker data and new stack identity before removing Compose projects", async () => {
    projects[0].config.services = [{ id: "stack", name: "Stack", cwd: ".", type: "compose", file: "compose.yaml" }];
    vi.mocked(confirmAction).mockResolvedValue(false);
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Remove project Example" }));
    expect(confirmAction).toHaveBeenCalledWith(expect.stringContaining("existing Docker data is not reused automatically"), "Remove project?");
    expect(api.removeProject).not.toHaveBeenCalled();
  });

  it("selects a remaining project after removing the selected one", async () => {
    projects.push({ ...projects[0], id: "two", name: "Remaining", path: "/remaining" });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Example" }));
    await user.click(screen.getByRole("button", { name: "Remove project Example" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Remaining" })).toHaveClass("current"));
  });
});
