export type ServiceState =
  "stopped" | "starting" | "running" | "ready" | "stopping" | "failed";
export type VpnRoute = { profileId: string; remoteHost: string; remotePort: number; localPort: number };
export type ServiceDefinition = {
  id: string;
  name: string;
  cwd: string;
  envFile?: string;
  healthCheck?: { url: string };
  ports?: number[];
  url?: string;
  dependsOn?: ServiceRef[];
} & ({ type: "command"; command: string; vpn?: { profileId: string; image: string }; vpnRoutes?: VpnRoute[] } | { type: "compose"; file: string });
export type Snapshot = {
  state: ServiceState;
  runId?: string;
  pid?: number;
  exitCode?: number;
  startedAt?: string;
  health: string;
  error?: string;
  activity?: string;
};
export type Project = {
  id: string;
  name: string;
  path: string;
  revision: string;
  config: { schemaVersion: 1; services: ServiceDefinition[] };
  trusted: string[];
  runtime: Record<string, Snapshot>;
};
export type LogRecord = {
  seq: number;
  runId: string;
  timestamp: string;
  stream: "stdout" | "stderr" | "system";
  text: string;
  truncated: boolean;
};
export type LogPage = {
  records: LogRecord[];
  nextCursor: string;
  truncated: boolean;
};
export type AiStatus = {
  enabled: boolean;
  endpoint: string;
  clients: {
    id: string;
    name: string;
    projects: string[];
    projectRoots: string[];
    control: boolean;
    edit: boolean;
  }[];
  error?: string;
};
export type VpnPortMapping = { host: number; container: number };
export type VpnImport = { name: string; configPath: string; username: string; password: string; keyPassphrase?: string; publishedPorts: VpnPortMapping[]; outboundSubnets?: string[]; healthTarget: string; dnsAddress?: string; healthIcmpTarget?: string };
export type VpnProfile = { id: string; name: string; publishedPorts: VpnPortMapping[]; outboundSubnets: string[]; healthTarget: string; dnsAddress?: string; healthIcmpTarget?: string; state: string; authChallenge?: { id: string; prompt: string } | null; error?: string | null; users: ServiceRef[] };
export type Workspace = {
  vpns?: VpnProfile[];
  projects: Project[];
  groups: WorkspaceGroup[];
  ai: AiStatus;
  uncleanShutdown: boolean;
};
export type ServiceRef = { projectId: string; serviceId: string };
export type NetworkTraffic = {
  projectId: string; serviceId: string; runId: string; profileId: string;
  localPort: number; remoteHost: string; remotePort: number;
  txBytes: number; rxBytes: number; activeConnections: number;
};
export type WorkspaceGroup = {
  id: string;
  name: string;
  services: ServiceRef[];
};
export const stopped: Snapshot = { state: "stopped", health: "notConfigured" };
export const serviceActive = (state?: string) =>
  ["starting", "running", "ready", "stopping"].includes(state ?? "");
export const stateLabel = (state: string) =>
  state.charAt(0).toUpperCase() + state.slice(1);

export type PortListener = { port: number; pid: number; name: string; identity: string; managed?: ServiceRef };
export type RunSummary = { projectId: string; serviceId: string; runId: string; command: string; startedAt: string; endedAt?: string; state: string; exitCode?: number; error?: string; truncated: boolean };
export type RunRecord = RunSummary & { records: LogRecord[] };
export type Diagnostics = { projectId: string; serviceId: string; runId?: string; state: Snapshot; summary: string[]; records: LogRecord[]; nextCursor: string; truncated: boolean; redacted: boolean };
