import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask, open } from "@tauri-apps/plugin-dialog";
import type {
  AiStatus,
  LogPage,
  Project,
  ServiceDefinition,
  Workspace,
  WorkspaceGroup,
  ServiceRef,
} from "./types";

export const isDesktop = () => "__TAURI_INTERNALS__" in window;
export const confirmAction = (message: string, title: string) =>
  isDesktop()
    ? ask(message, {
        title,
        kind: "warning",
        okLabel: "Continue",
        cancelLabel: "Cancel",
      })
    : Promise.resolve(window.confirm(message));
export function errorMessage(error: unknown): string {
  if (typeof error === "object" && error && "message" in error)
    return String(error.message);
  return typeof error === "string" ? error : "Something went wrong. Try again.";
}
export async function pickProjectFolder(title = "Open a code folder") {
  if (!isDesktop())
    throw new Error("Open the desktop app to choose a project folder.");
  return open({
    directory: true,
    multiple: false,
    title,
  });
}
export const api = {
  onWorkspace: (callback: (workspace: Workspace) => void) =>
    listen<Workspace>("workspace-changed", (event) => callback(event.payload)),
  onLogs: (callback: () => void) => listen("logs-changed", callback),
  workspace: (): Promise<Workspace> =>
    isDesktop()
      ? invoke("workspace")
      : Promise.resolve({
          projects: [],
          groups: [],
          ai: {
            enabled: false,
            endpoint: "http://127.0.0.1:47831/mcp",
            clients: [],
          },
          uncleanShutdown: false,
        }),
  openProject: (path: string): Promise<Project> =>
    invoke("open_project", { path }),
  removeProject: (projectId: string): Promise<void> =>
    invoke("remove_project", { projectId }),
  saveGroup: (
    id: string | undefined,
    name: string,
    services: ServiceRef[],
  ): Promise<WorkspaceGroup> =>
    invoke("save_group", { id: id ?? null, name, services }),
  removeGroup: (id: string): Promise<void> => invoke("remove_group", { id }),
  saveService: (
    projectId: string,
    definition: ServiceDefinition,
    expectedRevision: string,
  ): Promise<Project> =>
    invoke("save_service", { projectId, definition, expectedRevision }),
  trustService: (
    projectId: string,
    serviceId: string,
    expectedRevision: string,
  ): Promise<void> =>
    invoke("trust_service", { projectId, serviceId, expectedRevision }),
  control: (
    projectId: string,
    serviceId: string,
    action: "start" | "stop" | "restart",
  ): Promise<void> =>
    invoke("control_service", { projectId, serviceId, action }),
  logs: (
    projectId: string,
    serviceId: string,
    cursor?: string,
  ): Promise<LogPage> => invoke("read_logs", { projectId, serviceId, cursor }),
  remove: (
    projectId: string,
    serviceId: string,
    expectedRevision: string,
  ): Promise<void> =>
    invoke("remove_service", { projectId, serviceId, expectedRevision }),
  clearLogs: (projectId: string, serviceId: string): Promise<void> =>
    invoke("clear_logs", { projectId, serviceId }),
  setAi: (enabled: boolean): Promise<AiStatus> => invoke("set_ai", { enabled }),
  createClient: (
    name: string,
    projects: string[],
    control: boolean,
    edit: boolean,
    projectRoots: string[] = [],
  ): Promise<{ token: string; id: string; proxyPath: string }> =>
    invoke("create_client", { name, projects, control, edit, projectRoots }),
  revokeClient: (id: string): Promise<void> => invoke("revoke_client", { id }),
};
