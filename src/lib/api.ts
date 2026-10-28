import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask, open } from "@tauri-apps/plugin-dialog";
import type {
  AiStatus,
  VpnImport, VpnPortMapping, VpnProfile,
  LogPage,
  Project,
  ServiceDefinition,
  Workspace,
  WorkspaceGroup,
  ServiceRef,
  PortListener, RunSummary, RunRecord, Diagnostics, NetworkTraffic,
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
export async function pickVpnConfig() {
  if (!isDesktop()) throw new Error("Open the desktop app to import an OpenVPN profile.");
  return open({ multiple: false, directory: false, filters: [{ name: "OpenVPN", extensions: ["ovpn"] }] });
}
export const api = {
  networkTraffic: (): Promise<NetworkTraffic[]> => invoke("network_traffic"),
  listVpnProfiles: (): Promise<VpnProfile[]> => invoke("list_vpn_profiles", {}),
  importVpnProfile: (input: VpnImport): Promise<VpnProfile> => invoke("import_vpn_profile", { input }),
  updateVpnNetwork: (id: string, publishedPorts: VpnPortMapping[], outboundSubnets: string[]): Promise<VpnProfile> => invoke("update_vpn_network", { id, publishedPorts, outboundSubnets }),
  connectVpn: (id: string): Promise<VpnProfile> => invoke("connect_vpn", { id }),
  respondVpnChallenge: (id: string, challengeId: string, response: string): Promise<void> => invoke("respond_vpn_challenge", { id, challengeId, response }),
  disconnectVpn: (id: string): Promise<void> => invoke("disconnect_vpn", { id }),
  removeVpnProfile: (id: string): Promise<void> => invoke("remove_vpn_profile", { id }),
  pullContainerImage: (image: string): Promise<void> => invoke("pull_container_image", { image }),
  inspectPorts: (projectId: string, serviceId: string): Promise<PortListener[]> => invoke("inspect_ports", {projectId, serviceId}),
  killPort: (projectId: string, serviceId: string, listener: {port:number;pid:number;identity:string}): Promise<void> => invoke("kill_port", {projectId, serviceId, ...listener}),
  runHistory: (projectId:string, serviceId:string):Promise<RunSummary[]> => invoke("run_history", {projectId,serviceId}),
  runDetails: (projectId:string, serviceId:string, runId:string):Promise<RunRecord> => invoke("run_details", {projectId,serviceId,runId}),
  diagnose: (projectId:string, serviceId:string):Promise<Diagnostics> => invoke("diagnose_service", {projectId,serviceId}),
  openUrl: (projectId:string, serviceId:string):Promise<void> => invoke("open_service_url", {projectId,serviceId}),
  controlSequence: (targets:ServiceRef[], action:"start"|"stop"|"restart"):Promise<void> => invoke("control_sequence", {targets,action}),
  cancelSequence: ():Promise<void> => invoke("cancel_sequence"),
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
  moveService: (
    service: ServiceRef,
    groupId: string | undefined,
  ): Promise<void> =>
    invoke("move_service", { service, groupId: groupId ?? null }),
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
