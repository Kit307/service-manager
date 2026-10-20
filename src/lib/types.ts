export type ServiceState =
  | "stopped"
  | "starting"
  | "running"
  | "ready"
  | "stopping"
  | "failed";
export type ServiceDefinition = {
  id: string;
  name: string;
  cwd: string;
  envFile?: string;
  healthCheck?: { url: string };
} & ({ type: "command"; command: string } | { type: "compose"; file: string });
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
export type Workspace = {
  projects: Project[];
  ai: AiStatus;
  uncleanShutdown: boolean;
};
export const stopped: Snapshot = { state: "stopped", health: "notConfigured" };
