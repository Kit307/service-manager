# Service Manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Execute inline and sequentially; delegate only when the user explicitly authorizes it.

**Goal:** Build a lightweight desktop application that opens project folders, saves editable service definitions, runs and stops their services, displays live logs, and lets local Codex and Claude Code clients operate the same manager through MCP.

**Architecture:** A single Tauri application hosts a Rust manager. React uses validated Tauri commands and events; authenticated local MCP connections call the same manager. Service processes and Docker Compose resources are owned by that manager, independently of the selected UI project.

An AI-facing proxy script invokes a non-GUI mode of the same Rust executable, connects as an authenticated MCP client, and forwards the manager's logs into AI shell-tool output. It does not own service processes or create a second backend.

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vite, pnpm, native CSS, and the official Rust MCP SDK. Use the native macOS webview; do not bundle Chromium or a Node.js backend.

## สรุปแผนภาษาไทย

เริ่มบน macOS Apple Silicon ใช้ Tauri + Rust + React + TypeScript เปิดโฟลเดอร์โค้ดแล้วเพิ่ม service พร้อมคำสั่งรันได้ เก็บค่าที่ `.service-manager/project.json` เปิดโฟลเดอร์เดิมแล้วค่าเดิมยังอยู่ แต่ไม่เริ่มรันเอง หน้าแอปมีรายการโปรเจกต์และ service ทางซ้าย ปุ่มควบคุมตรงกลาง และ log สดด้านล่าง

Rust เป็นเจ้าของ process และ log เพียงชุดเดียว ทั้งหน้าแอปและ AI เรียกตัวจัดการนี้ผ่านคนละช่องทาง ให้ Codex และ Claude Code ต่อ MCP ผ่าน HTTP บนเครื่องพร้อม token และสิทธิ์แยกตามโปรเจกต์ ผู้ใช้เปิดสิทธิ์ให้ AI อ่าน log, ควบคุม หรือเพิ่มและแก้คำสั่งได้จากหน้าแอป

เพิ่ม proxy script สำหรับ AI ที่ใช้ shell tool: AI เรียก script เพื่อสั่งตัวจัดการเดิมให้รัน service แล้ว script ส่ง log กลับทาง stdout ให้ AI อ่าน พร้อมกับที่ log ขึ้นในแอป การหมดเวลาของ shell tool หรือการปิด proxy ไม่หยุด service และไม่เปิด service ซ้ำ

ลำดับงานคือ (1) แอปพื้นฐาน (2) config และฟอร์มเพิ่ม service (3) trust และ Run/Stop/Restart (4) log สดแบบจำกัดหน่วยความจำ (5) health check และการปิดแอป (6) Docker Compose (7) หน้าแอปครบ flow (8) MCP และสิทธิ์ (9) ทดสอบ Codex/Claude Code จริง (10) แพ็ก `.app` พร้อมวัดขนาด เวลาเปิด และ RAM

เลือกใช้ JSON แทนฐานข้อมูล ไม่ฝังแชต AI ไม่ทำ cloud/SSH/deploy และไม่เพิ่ม terminal เต็มรูปแบบในรุ่นแรก ยังไม่มีคำถามด้านรูปแบบแอปที่ต้องตัดสินใจเพิ่ม หากขั้นลงมือต้องแก้ค่าของ AI client เดิม ใช้ credentials หรือกระทบระบบร่วม จะถามเฉพาะเรื่องนั้นพร้อมระบุเป้าหมายให้ชัดเจน

รายละเอียดต่อไปนี้เป็นสัญญาพฤติกรรม ลำดับงาน และเกณฑ์ตรวจรับสำหรับผู้พัฒนา ยังไม่ใช่หลักฐานว่าตัวแอปทำงานแล้ว

## Global Constraints

- Initial supported release: local macOS 13 or later on Apple Silicon. Other operating systems and Intel builds require their own lifecycle tests before support is claimed.
- Use the selected stack: Tauri + Rust + React + TypeScript.
- No automatic execution when opening a folder, restoring recent projects, or loading configuration.
- UI and MCP share one manager and the same authorization, configuration, lifecycle, and log paths.
- Never start another manager per MCP connection.
- Preserve project files and user work. Never remove volumes, kill processes by port/name, or silently overwrite concurrent configuration edits.
- No embedded AI chat, model API key, cloud service, remote deployment, or automatic installation of project dependencies.
- Runtime commands execute with the local user's permissions. This application is not a code sandbox.
- No new questions are required to write or begin this plan. Pause only at the material stop conditions below.
- All shell commands use `rtk`; when exact output or no native wrapper is needed, use `rtk proxy`.
- Pin resolved dependency versions in lockfiles during scaffolding, after checking current mutually compatible releases. This plan does not assert unverified patch versions.
- Keep credentials, real project logs, absolute private project paths, generated build artifacts, and MCP client configuration out of repository commits.
- This is a plan, not an implemented application. Do not claim client compatibility, cleanup reliability, or performance until the corresponding verification runs.

## 1. Product decisions made

| Question | Decision for the first release |
|---|---|
| App name | Service Manager; bundle identifier `dev.servicemanager.desktop` |
| Platform | macOS 13 or later on Apple Silicon; record the actual tested OS version |
| UI | React + TypeScript, ordinary CSS, system fonts, system light/dark preference |
| Layout | Projects/services on the left; service details and controls in the main pane; resizable log pane below |
| Project discovery | Explicit Open Folder with native folder picker; keep a recent-project list |
| Persistence | `.service-manager/project.json` in each project; native app-data directory for recent projects, trust, and client credentials |
| Project requirement | Any readable folder; the app does not require or initialize Git in folders it manages |
| Service types | Foreground shell command and an explicit Docker Compose type |
| Command shell | `/bin/zsh -lc <command>` on macOS; working directory is set by the manager, not interpolated into command text |
| Secrets | Optional project-local env-file reference; no env-file contents returned through MCP or stored in shareable JSON |
| Logs | Live stdout/stderr; bounded in-memory history, no default persistent log files |
| Readiness | Optional loopback HTTP health check; otherwise status is running, never automatically ready |
| AI clients | Codex local app/CLI and Claude Code local client; test each intended surface independently |
| AI shell-tool output | A local proxy wrapper forwards logs from the shared manager through an authenticated MCP client; no second process manager |
| MCP transport | Streamable HTTP at `http://127.0.0.1:47831/mcp`, authenticated per-client bearer token |
| MCP port conflicts | Report connection feature unavailable and offer a manually chosen port; never contact, kill, or silently replace the occupying process |
| AI authorization | Per-client project grants: read, control, and optional edit. Only the desktop UI can grant or expand permissions |
| App lifecycle | Closing the window hides it while services continue; tray/menu can reopen it. Quit offers Stop services and quit or Cancel |
| Startup policy | No launch-at-login and no automatic service restart |
| Group execution | Defer Start All and dependency graphs; one folder can already hold many independently controlled services |
| Distribution | Local `.app` build first; public distribution, signing, notarization, and automatic updates are separate approvals/work |

### User flow

1. Open the app and select Open Folder.
2. Load `.service-manager/project.json`; if absent, show Add Service without writing files yet.
3. Add a name, relative working directory, and command, or select the Compose type and file.
4. Save, review executable configuration, and trust it locally before first execution.
5. Run, inspect status, view logs, stop, and restart from the same service view.
6. Reopen the folder later to restore definitions without starting anything.
7. Open AI Connections, choose allowed projects and permissions, and copy the client-specific MCP setup.
8. Ask Codex or Claude Code to list services, start one, check readiness, or read logs. All changes appear in the desktop UI.

## 2. Behavior and invariants

### Project identity and configuration

- Canonicalize opened roots and deduplicate by canonical path. Assign a local project UUID in app state; do not persist runtime identity or machine-specific paths into shareable project JSON.
- Service IDs are stable within the project. Names are labels and may change without redirecting runtime operations.
- A new folder has empty definitions. Create `.service-manager/` only on explicit save.
- Refuse writes through symlinked configuration files/directories. Require canonical service directories, env files, and Compose files to remain inside the authorized root. A selected root may itself have been reached through a symlink; authorize its canonical target explicitly.
- Treat the service command as intentionally executable text. Passing the directory separately prevents accidental shell interpolation, but does not make an untrusted command safe.
- Use a tagged union for command versus Compose configuration. Reject unknown fields, duplicate IDs, invalid paths, empty commands, unsupported schema versions, oversized files, and malformed health-check URLs before changing runtime state.
- Configuration limit: 1 MiB. Preserve malformed or unsupported files and show their error; never replace them with defaults.
- Every read returns a SHA-256 revision of the exact file bytes. An absent file has revision `absent`. Every save supplies the expected revision. Under the shared project write lock, re-read and compare before writing a same-directory temporary file and atomically replacing the original.
- Detect external edits on focus refresh and before every mutation/start. Do not save over external edits. Emit `CONFIG_CONFLICT` and offer Reload; preserve the unsaved UI draft.
- The revision check protects clients of this manager, not arbitrary unrelated writers racing an OS rename. Document that external editors must reload/retry if they write concurrently; do not promise a filesystem transaction across uncooperative processes.
- Local trust applies to executable settings: type, command, directory, Compose file, env-file reference, and health-check target. External changes invalidate affected trust. Editing a running definition applies to its next run; retain the launch snapshot for stopping the current run.
- MCP edit permission is a locally granted authorization to change executable definitions for that project. Without that permission, return `PERMISSION_DENIED`; AI cannot authorize itself. New external file content still requires local review. UI must explain the scope of edit permission before it is granted.

### Example shareable configuration

```json
{
  "schemaVersion": 1,
  "services": [
    {
      "id": "api",
      "name": "API",
      "type": "command",
      "cwd": "backend",
      "command": "go run ./cmd/api",
      "envFile": ".env.local",
      "healthCheck": { "url": "http://127.0.0.1:8080/health" }
    },
    {
      "id": "web",
      "name": "Frontend",
      "type": "command",
      "cwd": "frontend",
      "command": "pnpm dev"
    },
    {
      "id": "dependencies",
      "name": "Local dependencies",
      "type": "compose",
      "cwd": ".",
      "file": "compose.yaml"
    }
  ]
}
```

All `cwd`, `envFile`, and Compose `file` paths are relative to the project root. An env file must already exist; the app neither creates nor overwrites it. Actual project configuration and secret files are not part of this implementation repository's examples.

### Service lifecycle

```text
stopped/failed -> starting -> running -> ready (only with a successful health check)
starting/running/ready -> stopping -> stopped
spawn failure or unexpected exit -> failed
health check timeout/failure -> running with explicit health error
health recovers -> ready
```

- Serialize mutations per service; never hold a global manager lock during process I/O, health polling, or waiting for exit.
- Each launch gets a new `runId`. Exit notifications, stdout readers, and health results include it; old tasks cannot overwrite a new run's state.
- Start on an active service returns its current snapshot without spawning a duplicate. Stop on stopped returns stopped. A concurrent restart returns `OPERATION_IN_PROGRESS`; do not silently queue repeated restarts.
- Start/stop/restart return an operation ID and immediate snapshot; clients poll status for the outcome. Acknowledged is not ready or stopped. Record the terminal operation outcome until the next mutation so clients can correlate it.
- A command runs in its own process group and must stay in the foreground. Stop sends SIGTERM to the owned group, waits up to 5 seconds, then SIGKILL if needed and waits for reaping. Retain ownership until the group and child handles are accounted for.
- If a shell/leader exits while children remain in the managed group, reconcile and clean up that group before allowing another run. Do not treat leader exit alone as complete cleanup.
- Refuse to mark stopped or report successful quit while cleanup remains uncertain. Show an actionable cleanup error.
- Processes that deliberately detach into another session and catastrophic app/OS termination are outside guaranteed cleanup. Do not adopt or kill an old PID from disk without verified ownership; on unclean startup show a warning and guide inspection. No automatic PID-based recovery in this release.
- Clean command exit is displayed as stopped with its exit code; a command with health checking that exits before readiness is failed. Nonzero or signalled unexpected exits are failed.
- Do not install tools or dependencies on Start. Return missing-executable or missing-env-file diagnostics with a readable summary.

### Shell environment

- `/bin/zsh -lc` supports normal developer tool initialization without inventing a separate PATH. Start it in the chosen directory with the inherited environment and an optional parsed env file.
- Parse env files with a dotenv parser without shell evaluation; file values override inherited values only for that child.
- A login shell can have user-specific configuration, delays, or missing interactive-only tool setup. Keep Stop usable during startup and show missing-tool hints. Do not source arbitrary additional shell files or guess secret values.
- UI diagnostics show tool availability and the selected executable path, never a full environment dump. Test launch from Finder as well as a terminal.

### Compose lifecycle

- Use a typed Compose definition, not heuristic parsing of arbitrary command strings.
- Run Docker with argument arrays: `docker compose --project-name <owned-name> --project-directory <cwd> --file <file> up --no-color`. Do not use `-d`, `--build`, or `--pull` automatically.
- The owned name is derived from a local project UUID and service ID and remains stable on this machine. Never adopt another Compose project solely because its display name matches.
- Preflight Docker availability, daemon reachability, Compose configuration, and the chosen project's ownership labels. Compose config can interpolate secrets: capture preflight output privately and return a redacted error summary, not the expanded configuration.
- Reject nonempty pre-existing resources for the selected Compose project unless local ownership was recorded and verified. Require review for configuration using explicit `container_name` or external resources; do not promise those resources are isolated by project naming.
- Poll Compose container state. Running means expected containers are running; ready requires an app-configured HTTP health check or healthy Docker health checks for all required long-running containers. Exited one-shot containers are successful only if their configuration and exit code indicate that role.
- If the Compose runner exits unexpectedly, inspect owned container state rather than inferring that containers stopped. Show failure if reconciliation cannot establish their state.
- Stop executes `docker compose ... stop --timeout 5` for the launch snapshot, then reconciles owned containers and runner. Verify owned containers stopped before success; retain an error if Docker is unavailable.
- Never call `down`, prune, remove volumes, stop the daemon, or alter unrelated containers. Restart reuses the same ownership and waits for successful stop.

### Health checks

- Optional HTTP/HTTPS GET, with loopback targets only, no URL credentials, no redirects, 2-second request timeout, and 1-second poll interval. Constrain both hostname resolution and the actual connection to loopback; `localhost` resolving outside loopback is rejected.
- A 2xx result is healthy. At 30 seconds without success, expose a startup health error while keeping the process running. Continue checks; a later success becomes ready.
- A ready service with a failed later health check returns to running with unhealthy status. Do not auto-restart it.
- Cancel checks on stop and ignore results whose run ID no longer matches.

### Logs and event load

- One log record contains project ID, service ID, run ID, monotonic sequence, UTC timestamp, stdout/stderr/system stream, and text.
- Retain at most 5,000 records and 2 MiB per service, plus a 32 MiB global cap across all services. Evict oldest retained records when either cap is reached. Keep counters/cursors consistent across eviction and restart.
- Limit one record to 16 KiB and mark truncation; bound the raw pending buffer too, including output without newlines and invalid UTF-8.
- Batch UI log events every 100 ms, at most 100 records and 128 KiB per event. Use bounded channels; a slow renderer cannot stall child pipe draining. Notify of dropped events and recover retained data with a cursor.
- Log reads default to 200 records, maximum 1,000 and 128 KiB per response. Return `nextCursor`, `hasMore`, and `truncated`; an evicted cursor is reported instead of silently implying complete history.
- Clear Logs only clears retained history, never a project file or the running process. Sequence numbers remain monotonic.
- Render output as text. Strip ANSI control codes for this release, never interpret terminal commands or HTML, and preserve stderr distinction.
- App diagnostics exclude tokens, env contents, and command environment. Application output may itself contain sensitive values; users authorize project log access explicitly. Do not claim perfect automatic log redaction.

## 3. MCP contract and security

Use the official Rust SDK (`rmcp`) and its Streamable HTTP implementation with Axum. Do not implement JSON-RPC, MCP sessions, negotiation, or framing by hand. Select compatible crate versions and features in Task 8.

Start the HTTP listener only when AI Connections is enabled. Bind exactly `127.0.0.1`; no LAN binding, reverse proxy, tunnel, public endpoint, or OAuth service in this release. Local HTTP is an intentional loopback-only choice.

Authenticate every MCP request and every stream/session continuation with a 256-bit random per-client bearer token. Store only its hash and grants in a private app-data file with mode `0600`; show a new token once for copying. Compare in constant time, never accept a token in the URL, never log it, and never expose client credentials through MCP. Rotation creates a new token and revokes the old one. Disabling a client also closes its sessions/streams.

Validate Host against the bound endpoint. Reject browser Origin headers by default and do not enable wildcard CORS. MCP inspection from a browser is a developer-only test configuration, never a production security bypass. Apply a 1 MiB request limit and a bounded per-client request concurrency limit of 8. Return a clear limit error without retrying mutating calls.

Permissions are stored by local project ID, not whichever folder is selected in React. Read is required for list/status/log/config; control permits lifecycle calls on trusted definitions; edit permits configuration updates. Both control and edit imply read, but edit does not imply control. The UI displays these grants and allows revocation. MCP has no tool for adding authorized roots, changing grants, trusting external file changes, issuing tokens, or shutting down the application.

### Tools

| Tool | Input | Output | Permission |
|---|---|---|---|
| `list_projects` | none | Only authorized opened project IDs and display names | read |
| `list_services` | `projectId` | Definitions without secret values, revision, trust state, runtime snapshots | read |
| `get_service_status` | `projectId`, `serviceId` | Snapshot, readiness, latest operation outcome | read |
| `get_project_config` | `projectId` | Validated config and revision; no env-file contents or private runtime settings | read |
| `upsert_service` | `projectId`, complete service definition, `expectedRevision` | New revision and definition; no automatic start | edit |
| `start_service` | `projectId`, `serviceId` | Operation ID and current snapshot | control |
| `stop_service` | `projectId`, `serviceId` | Operation ID and current snapshot | control |
| `restart_service` | `projectId`, `serviceId` | Operation ID and current snapshot | control |
| `read_logs` | `projectId`, `serviceId`, optional cursor/runId/stream/limit | Bounded records and cursor metadata | read |

Upsert handles adding and updating; no generic command execution, file read, file deletion, config replacement, or shell tool is exposed. UI service removal is allowed only after a successful stop, asks for confirmation, and removes the definition only. It is not included as an AI tool initially.

Use structured results and safe error codes: `PROJECT_NOT_AUTHORIZED`, `SERVICE_NOT_FOUND`, `CONFIG_INVALID`, `CONFIG_CONFLICT`, `TRUST_REQUIRED`, `PERMISSION_DENIED`, `OPERATION_IN_PROGRESS`, `PROCESS_START_FAILED`, `STOP_FAILED`, `DOCKER_UNAVAILABLE`, `LOG_CURSOR_EXPIRED`, and `MCP_UNAVAILABLE`. Protocol/auth errors remain MCP/HTTP errors; domain tool failures use the SDK's tool error mechanism. Do not return raw internal errors containing secrets.

Mark only observational tools read-only. Mark lifecycle/config writes as mutations. Tool annotations help clients but do not replace server-side authorization. Describe logs as untrusted output. Do not retry start/restart because an HTTP client timed out: query status first.

### Client setup choices

- Provide an AI Connections screen with endpoint, permission summary, token generation/revocation, and copyable client-specific instructions. The app never overwrites existing Codex or Claude configuration.
- Codex: Streamable HTTP supports `url` and bearer-token configuration; generate instructions for a private user configuration entry, with an environment-variable token option. GUI clients may not inherit terminal environment variables, so document private static-header setup as an explicit alternative after testing the actual target surface.
- Claude Code: use HTTP MCP configuration with an Authorization header and environment-variable expansion where supported. Keep real tokens out of project `.mcp.json`, repositories, screenshots, and evidence.
- New connections can require a new client session/reload. Explain app-offline and token-revoked errors clearly.
- Local Codex app/CLI and Claude Code are the acceptance targets. Claude web/remote connectors and cloud Codex are excluded because their ability to reach this machine's loopback is different. Claude Desktop support is not implied by a passing Claude Code test.
- Standard MCP integration tests can run with synthetic credentials. Changes to a person's existing AI client settings require explicit execution authorization; absent that, supply setup and record the real-client checks as pending.

### Proxy script for AI shell tools

The user also requires log output to flow through a script into the AI tool's captured output. Support this as a second access path to the same manager, not a second execution engine:

```text
AI shell tool -> local proxy script -> Rust proxy mode -> authenticated MCP
                                                          |
Desktop UI <--- status + live logs <--- shared manager -> service process
AI shell tool <--- stdout log records <--- Rust proxy mode <--- read_logs
```

Implement `service-manager proxy run --project <project-id> --service <service-id> [--duration <seconds>]` and `service-manager proxy logs --project <project-id> --service <service-id> [--run <run-id>] [--cursor <cursor>] [--follow] [--duration <seconds>]`. These are planned command interfaces, not currently installed commands.

- Parse proxy mode before Tauri GUI initialization so a proxy call never opens a window, registers another desktop instance, or creates a manager. Use the official Rust MCP SDK client against the configured loopback endpoint.
- Run calls `start_service` once, captures its returned run ID, and follows that run's output for up to 15 seconds and 128 KiB. If the service is already active, attach to its existing run without duplicating it. Query status rather than retrying the start if a response is lost.
- Logs defaults to a bounded recent snapshot. `--follow` polls `read_logs` with cursors; follow duration defaults to 15 seconds and can be explicitly set between 1 and 60 seconds. Run uses the same duration limit. Stop sooner on terminal run state after draining retained records, byte limit, interrupt, or disconnected output.
- Forward service records as tagged text on stdout. Put proxy diagnostics and a final structured JSON summary on stderr containing project/service/run IDs, last state, operation outcome when available, reason capture ended, truncation information, and `nextCursor`. Never print credentials.
- A successful bounded capture can return while the service is still running. Exit code 0 means the proxy request/capture succeeded, not that a long-lived service finished or became ready. Use 1 for a known failed service/operation, 2 for invalid arguments, 3 for unavailable backend/transport, 4 for authentication/authorization failure, and 130 for an interrupt. Pair every exit with a truthful summary when stderr remains available.
- Timeout, Ctrl-C, a closed pipe, or killing the AI tool's proxy process detaches the observer only. It must not send Stop, kill the managed process, or disconnect the desktop's log collectors. Starting a service remains an explicit side effect even if its caller disappears immediately afterward.
- Use returned cursors to request later output; never restart a service to retrieve more logs. Deduplicate record sequence numbers and surface history eviction. Logs remain untrusted application output.
- AI Connections offers Create Proxy Script. It explicitly creates a wrapper and dedicated credential profile in private app-data storage, with restrictive permissions and paths correctly quoted for spaces. The wrapper forwards arguments unchanged to the bundled executable. Show/copy its absolute invocation; do not require a global PATH installation.
- Store the proxy's bearer token in a separate `0600` credential file associated with its grant profile. The server continues to store the token hash. This file grants the same capability as private client credentials; same-user arbitrary code is outside the protection boundary. Never pass tokens in argv, expose them in script source, or put machine-specific generated wrappers/credentials into shareable project config.
- The proxy profile receives only the locally selected project read/control grants. It cannot silently inherit all projects or edit permissions. Revoking it terminates its sessions. It cannot enable AI Connections or issue credentials itself.
- Missing app, disabled AI Connections, stale wrapper executable path, missing credential file, port conflict, or revoked token produce an actionable error. Never auto-launch a hidden manager or downgrade authentication.
- Explain that an AI client must actually execute the proxy command and retain/read its output. A script cannot inject logs into an idle chat on its own; future reads occur through explicit tool calls.

## 4. Interfaces and ownership

Wire DTOs use camelCase via Serde. `src/lib/types.ts` mirrors the Rust DTOs and has contract fixtures checked in both test suites. Avoid a new schema-codegen tool until manual DTO maintenance demonstrably causes drift.

```ts
type ServiceKey = { projectId: string; serviceId: string };
type ServiceState = "stopped" | "starting" | "running" | "ready" | "stopping" | "failed";
type CommandDefinition = {
  id: string; name: string; type: "command"; cwd: string; command: string;
  envFile?: string; healthCheck?: { url: string };
};
type ComposeDefinition = {
  id: string; name: string; type: "compose"; cwd: string; file: string;
  envFile?: string; healthCheck?: { url: string };
};
type ServiceDefinition = CommandDefinition | ComposeDefinition;
type ProjectConfig = { schemaVersion: 1; services: ServiceDefinition[] };
type ConfigRead = { config: ProjectConfig; revision: string; trustedServiceIds: string[] };
type OperationReceipt = { operationId: string; snapshot: ServiceSnapshot };
type OperationOutcome = {
  operationId: string; action: "start" | "stop" | "restart";
  status: "pending" | "succeeded" | "failed"; error?: { code: string; message: string };
};
type ServiceSnapshot = ServiceKey & {
  runId?: string; state: ServiceState; pid?: number; exitCode?: number;
  health: "notConfigured" | "checking" | "healthy" | "unhealthy";
  lastOperation?: OperationOutcome; error?: { code: string; message: string };
};
type LogRecord = ServiceKey & {
  runId: string; seq: number; timestamp: string;
  stream: "stdout" | "stderr" | "system"; text: string; truncated: boolean;
};
type LogQuery = {
  cursor?: string; runId?: string; stream?: LogRecord["stream"]; limit?: number;
};
type LogPage = { records: LogRecord[]; nextCursor: string; hasMore: boolean; truncated: boolean };
```

Rust modules expose concrete structs rather than single-implementation abstraction traits. Use `ServiceManager` in `manager.rs`, `ProjectStore` in `project.rs`, `LogStore` in `logs.rs`, and transport handlers that call them.

```text
ServiceManager::open_project(root: PathBuf) -> Result<ProjectSummary>
ServiceManager::list_projects(context: CallerContext) -> Result<Vec<ProjectSummary>>
ServiceManager::list_services(context: CallerContext, project_id: Uuid)
                             -> Result<ServiceList>
ServiceManager::get_config(context: CallerContext, project_id: Uuid) -> Result<ConfigRead>
ServiceManager::upsert_service(context: CallerContext, project_id: Uuid,
                              definition: ServiceDefinition, expected_revision: String)
                              -> Result<ConfigRead>
ServiceManager::start(context: CallerContext, key: ServiceKey) -> Result<OperationReceipt>
ServiceManager::stop(context: CallerContext, key: ServiceKey) -> Result<OperationReceipt>
ServiceManager::restart(context: CallerContext, key: ServiceKey) -> Result<OperationReceipt>
ServiceManager::status(context: CallerContext, key: ServiceKey) -> Result<ServiceSnapshot>
ServiceManager::read_logs(context: CallerContext, key: ServiceKey,
                         query: LogQuery) -> Result<LogPage>
```

These methods are async where they perform I/O. `ProjectSummary` contains the project UUID and display name; private UI reads may also include the canonical root. `ServiceList` contains the validated definitions, revision, trust state, and matching runtime snapshots described in the tool contract. `CallerContext` is created internally by the local UI transport or validated MCP authentication; it is never accepted from request JSON. Private UI-only operations manage trust, recent folders, service removal, quit, and AI client grants.

### Planned file structure

```text
src/
  App.tsx                         # app shell, project selection, shared UI state
  components/ProjectSidebar.tsx    # recent projects and service list
  components/ServiceEditor.tsx     # command/Compose forms and validation errors
  components/ServiceDetail.tsx     # status, lifecycle controls, readiness and errors
  components/LogPanel.tsx          # bounded text log display and filters
  components/AiConnections.tsx     # local client grants and setup instructions
  lib/api.ts                      # Tauri invoke/listen boundary, no process logic
  lib/types.ts                    # wire DTOs
  styles.css                      # layout, tokens, focus states, system theme
  test/                           # React tests and wire contract fixtures
src-tauri/
  src/main.rs                     # desktop entry
  src/lib.rs                      # assemble app state, commands, tray, shutdown
  src/model.rs                    # config, runtime, operation and error DTOs
  src/project.rs                  # validated atomic project and private state persistence
  src/trust.rs                    # executable fingerprints, client grants and revisions
  src/manager.rs                  # service registry and lifecycle orchestration
  src/process.rs                  # foreground commands and Unix process groups
  src/compose.rs                  # owned Docker Compose execution and reconciliation
  src/logs.rs                     # bounded records, cursors and output decoding
  src/health.rs                   # cancellable loopback readiness checks
  src/commands.rs                 # Tauri handlers and UI-only authorization actions
  src/mcp.rs                      # SDK tool handlers and loopback HTTP assembly
  src/proxy.rs                    # non-GUI MCP client mode and bounded stdout forwarding
  capabilities/default.json      # minimum local-webview permissions
  tauri.conf.json                 # desktop settings and restrictive content policy
  tests/                          # config, lifecycle, MCP and security integration tests
tests/fixtures/                   # isolated synthetic process and Compose fixtures
docs/ai-connections.md            # client setup verified against real versions
docs/verification.md              # acceptance evidence and limits
.agents/milestones/               # implementation milestone
.agents/tasks/                    # one record per implemented vertical task
.agents/evidence/                 # sanitized verification results only
```

Do not create empty future files. Create a file when its task implements it; fold a tiny module into its responsible neighbor if a separate file adds no clarity.

### Dependency budget

- Frontend runtime: React, React DOM, and Tauri APIs/plugins for native folder selection and required lifecycle integration. No router, Redux, terminal emulator, icon library, or component framework initially.
- Frontend dev: Vite, TypeScript, Vitest, Testing Library, and the minimum lint tooling from the selected React template.
- Rust: Tauri, Tokio, Serde/serde_json, UUID, thiserror, nix for Unix signalling, reqwest with a rustls TLS backend, dotenvy, SHA-256/random/constant-time helpers, and rmcp/Axum for standard MCP HTTP. Reuse versions/features already brought in by the scaffold where correct.
- Enable rmcp's client transport for proxy mode as well as its server transport. Parse the small fixed proxy argument surface directly unless the selected scaffold already includes a fitting CLI parser; do not create a general shell runner or extra Node/Python runtime.
- Rust dev: tempfile for isolated filesystem tests. Use standard library process fixtures or small committed shell fixtures rather than adding a second application runtime solely for tests.
- No SQLite, background daemon, message broker, generic plugin system, or database migration layer.

## 5. Implementation sequence

Execute one task at a time. Each task updates its `.agents/tasks/TASK-000N-<slug>.md` record using the user's template, records the focused proof, and ends with one focused commit if checks pass. Do not claim the application complete until Task 10 passes the delivery acceptance checks.

### Task 1: Runnable desktop shell and local build proof

**Files:** Create the Tauri React TypeScript scaffold, lockfiles, `src/App.tsx`, `src/styles.css`, `src-tauri/src/{lib,main}.rs`, `src-tauri/tauri.conf.json`, minimum capabilities, `.gitignore`, and `.agents/milestones/MILESTONE-0001-service-manager.md`.

**Consumes:** selected stack and repository design. **Produces:** an empty-state desktop app, native folder chooser, and reproducible local commands.

- [ ] Check current compatible Tauri 2/scaffold versions, Rust target/toolchain, pnpm, Xcode, and native build prerequisites before adding dependencies. Use Rust via its existing installation; do not install a second copy.
- [ ] Add the scaffold with package name `service-manager`, Vite frontend, and a single Tauri window. Set the bundle identifier from the decision table.
- [ ] Configure a restrictive production CSP, allow native commands only from the local application webview, disable remote navigation, and avoid exposing generic shell/fs execution plugins.
- [ ] Add scripts named `dev`, `build`, `typecheck`, `test`, `lint`, and `tauri`. Use `pnpm tauri dev` as the desktop development command.
- [ ] Prove Open Folder returns the native selected directory and Cancel leaves the empty state unchanged. Start no project processes.
- [ ] Run TypeScript checking, frontend build, Rust compile, and an actual desktop launch. Record commands and tool versions; commit the runnable shell.

Example UI acceptance check to implement with a mocked native boundary:

```tsx
it("keeps the empty state when folder selection is cancelled", async () => {
  vi.mocked(pickProjectFolder).mockResolvedValue(null);
  render(<App />);
  await userEvent.click(screen.getByRole("button", { name: "Open Folder" }));
  expect(screen.getByText("Open a project folder to begin")).toBeVisible();
  expect(api.openProject).not.toHaveBeenCalled();
});
```

The test setup must define/mock `pickProjectFolder` and `api.openProject` in `src/lib/api.ts`; use Testing Library and user-event in tests only.

### Task 2: Persistent projects and editable service definitions

**Files:** `model.rs`, `project.rs`, `commands.rs`, `src/lib/{types,api}.ts`, `ProjectSidebar.tsx`, `ServiceEditor.tsx`, `src-tauri/tests/project_config.rs`, shared contract fixtures.

**Consumes:** native folder selection. **Produces:** `open_project`, `get_config`, revision-aware service save, and recent-project persistence.

- [ ] Define the exact tagged config union and DTOs from Section 4. Add Serde strict-field validation and JSON fixtures accepted by both Rust and TypeScript tests.
- [ ] Write failure-first tests for missing config, reopen, malformed JSON, unknown version/fields, duplicate ID, root escape/symlink escape, env-file path escape, and stale expected revision.
- [ ] Implement canonical root identity, a native private app-data store, atomic same-directory writes, and optimistic revision conflict handling.
- [ ] Add service forms for command and Compose types, accessible field labels, unsaved-draft conflict handling, and an explicit Save action. Do not add command auto-detection.
- [ ] Verify add/edit/reopen, preservation of malformed files, and external-edit conflict handling. Confirm opening alone writes no project file and starts no service.
- [ ] Run focused config tests and React form tests, then typecheck and Rust compile; commit.

Concrete test scenario:

```text
open empty fixture -> revision absent, zero services, no project.json on disk
save web command with expectedRevision absent -> revision A
external editor changes command -> revision B
save previous UI draft with expectedRevision A -> CONFIG_CONFLICT
read file -> still contains the external editor's command, not the stale draft
```

### Task 3: Local trust and one shared command lifecycle

**Files:** `trust.rs`, `manager.rs`, `process.rs`, `model.rs`, `commands.rs`, `ServiceDetail.tsx`, `src-tauri/tests/{trust,process_lifecycle}.rs`, process fixtures.

**Consumes:** validated definitions and revisions. **Produces:** shared `ServiceManager`, start/stop/restart/status, local trust actions, and operation receipts.

- [ ] Create synthetic fixtures: long-running output loop, child-spawning foreground command, immediate exit 0, exit 7, ignored SIGTERM, and a child remaining after the shell leader exits. All use isolated temporary directories and ports.
- [ ] Write failing lifecycle tests for untrusted start, concurrent start, stop idempotence, restart sequencing, expected/failed exits, group cleanup, old-run result rejection, and stop escalation.
- [ ] Implement a per-service transition gate, run IDs, launch snapshots, asynchronous operations, and owned Unix process groups. Ensure pipe draining and reaping run independently of the renderer.
- [ ] Implement executable fingerprints and local review. Do not start restored services or trust new external definitions automatically.
- [ ] Add UI trust review, lifecycle controls, disabled states during transitions, readable errors, and status snapshots. Never report ready solely from process creation.
- [ ] Prove every fixture process/group is gone at test cleanup and unrelated fixture processes remain alive; run focused tests and commit.

Core concurrency assertion to implement against a shared test manager whose synthetic fixture appends a line to its private `starts.log` on each actual launch:

```rust
let (a, b) = tokio::join!(manager.start(ui.clone(), key.clone()),
                        manager.start(ui.clone(), key.clone()));
assert_eq!(a?.snapshot.run_id, b?.snapshot.run_id);
let starts = std::fs::read_to_string(fixture.path().join("starts.log"))?;
assert_eq!(starts.lines().count(), 1);
```

Wait with a bounded deadline for the fixture's initial start marker before checking it. Count actual fixture starts rather than adding a production-only spawn counter or abstraction.

### Task 4: Bounded live logs and reusable reads

**Files:** `logs.rs`, process output integration, `commands.rs`, `LogPanel.tsx`, log DTOs in `types.ts`, `src-tauri/tests/log_stream.rs`, `src/test/LogPanel.test.tsx`.

**Consumes:** run-scoped stdout/stderr. **Produces:** `LogStore`, `read_logs`, status/log events, and a live log panel.

- [ ] Write failing tests for stdout/stderr, newline-free floods, invalid UTF-8, huge records, cursor pagination, restart separation, eviction, clear, and a slow/disconnected subscriber.
- [ ] Implement incremental bounded decoding, timestamps, sequence IDs, per-service/global retention, cursor validation, and event batching exactly as specified.
- [ ] Render plain text, maintain independent selected-service log state, and add stream filter, pause auto-scroll, Clear Logs, and Copy visible selection using native browser capabilities where available.
- [ ] On event gaps, fetch retained data by cursor. Display a gap marker when history was evicted instead of fabricating completeness.
- [ ] Run flood fixtures while switching projects and pausing the renderer. Verify processes continue and memory remains bounded; commit.

Concrete retention test:

```text
emit >5,000 small lines -> at most 5,000 retained
emit >2 MiB for one service -> at most 2 MiB retained
emit a 20 MiB stream without newlines -> bounded pending decoder and truncation markers
read with evicted cursor -> LOG_CURSOR_EXPIRED with a recovery cursor
clear -> no retained lines; next emitted sequence is greater than the old maximum
```

### Task 5: Readiness, environment diagnostics, and shutdown

**Files:** `health.rs`, `process.rs`, `manager.rs`, Tauri lifecycle/tray assembly in `lib.rs`, `ServiceDetail.tsx`, `src-tauri/tests/{health,shutdown}.rs`.

**Consumes:** lifecycle and logs. **Produces:** honest readiness status, env-file support, tool diagnostics, and controlled quit behavior.

- [ ] Test loopback 200, 500, timeout, redirect, forbidden target, DNS rebinding/incorrect resolution, process exit during a check, and late result from a prior run.
- [ ] Implement cancellable health checks with the fixed thresholds from this plan. Check success before ready; display health failures without killing a valid running process.
- [ ] Add dotenv parsing and missing-tool diagnostics without exposing the environment or env-file values. Test a path containing spaces and a command containing intentional shell quoting.
- [ ] Implement window hide/reopen, tray/menu Quit, and Stop services and quit / Cancel dialog. Disable new mutations while quit cleanup is underway. Resume normal interaction if shutdown fails.
- [ ] Record clean/unclean shutdown privately. On unclean startup, warn without launching or killing stored PIDs.
- [ ] Test cancel quit, successful stop-and-quit, failed cleanup, and manual launch from Finder; commit.

Expected readiness sequence:

```text
foreground fixture starts + HTTP 500 -> running/unhealthy
same run returns HTTP 200 -> ready/healthy
HTTP later fails -> running/unhealthy
stop -> stopped and no remaining health task can restore ready
```

### Task 6: Owned Docker Compose services

**Files:** `compose.rs`, manager Compose integration, Compose editor fields, `tests/fixtures/compose.yaml`, `src-tauri/tests/compose_lifecycle.rs`.

**Consumes:** shared lifecycle, logs, validated roots, and launch snapshots. **Produces:** explicit Compose services managed without removing data or touching unrelated stacks.

- [ ] Write a harmless Compose fixture with a uniquely named project, no host mounts, no real secrets, an internal health check, and a separate unrelated fixture stack. Use an already approved local image; image pulls and daemon startup are explicit prerequisites, not hidden effects.
- [ ] First test command construction as argument arrays, ownership name stability, external-resource warnings, daemon errors, secret-safe preflight errors, and runner-versus-container reconciliation without requiring Docker.
- [ ] Implement up/stop/status using the exact project directory, Compose file, env file, and owned name from the launch snapshot.
- [ ] Exercise real Docker integration only against isolated fixtures with execution authorization. Confirm stop leaves the unrelated stack untouched and does not remove fixture volumes.
- [ ] Verify mixed command + Compose projects behave consistently in the UI. Missing Docker disables only Compose operations, not command services.
- [ ] Commit with real integration evidence or mark Docker acceptance explicitly pending if the runtime/authorization is unavailable. Do not call mocked command construction proof of container cleanup.

### Task 7: Complete desktop project flow

**Files:** `App.tsx`, all project/service components, `styles.css`, `api.ts`, `commands.rs`, React tests.

**Consumes:** real manager operations. **Produces:** usable end-to-end desktop flow for multiple opened projects.

- [ ] Compose the left project/service list, central editor/detail view, and lower resizable log pane. Use one selected-project state and explicit service keys in every call.
- [ ] Add recent-folder reopen, missing-folder handling, reload conflict actions, trust summary, and confirmed removal of stopped service definitions only.
- [ ] Preserve running services when selecting a different project. Never redirect an operation or log event based on current selection alone.
- [ ] Test keyboard navigation, visible focus, labelled controls, status text in addition to color, empty states, errors, and a usable compact window around 900 × 600.
- [ ] Run real command fixtures through Open Folder → Add → Save → Trust → Run → Logs → Stop → Reopen. Confirm no browser-only mock path is presented as desktop proof.
- [ ] Run React tests, typecheck, lint, frontend build, and focused Rust tests; commit.

### Task 8: Authenticated MCP with shared state

**Files:** `mcp.rs`, `trust.rs`, manager caller-context integration, `AiConnections.tsx`, `src-tauri/tests/mcp_{protocol,authorization,shared_state}.rs`.

**Consumes:** tested manager methods and log reads. **Produces:** the nine tools from Section 3 through a real MCP transport.

- [ ] Check compatible rmcp/Axum versions and official SDK examples. Add SDK transport/schema features only as required.
- [ ] Build a real protocol test using the SDK client: initialize, list tools, call a read tool, and reject an invalid argument. Authenticate at the HTTP boundary before serving protocol/session data.
- [ ] Add client issuance, hashed credential storage, per-project grants, token revocation, and minimum request/connection protections.
- [ ] Implement the tool contract as thin adapters to the existing manager. Do not add a transport-specific process registry or let request payloads supply caller identity.
- [ ] Test missing/wrong/revoked token, continuation authentication, Host/Origin rejection, unknown root, insufficient scope, oversized input, rate/concurrency bounds, config conflict, and old session use after revocation.
- [ ] Start from a UI caller, observe and stop through MCP; then start through MCP and observe/stop through UI. Switch UI project during a request and verify the original explicit project receives the action.
- [ ] Test that a log containing instructions cannot trigger any additional call inside the server. Verify read-only annotations and mutation annotations match actual side effects.
- [ ] Run focused protocol/security tests and the relevant lifecycle suite; commit.

Protocol-level acceptance scenario:

```text
client A: read+control on project P, read on project Q
client B: read on Q only
A starts P/web -> exactly one runId; desktop sees it
B reads P logs -> PROJECT_NOT_AUTHORIZED
A upserts P command -> PERMISSION_DENIED
desktop grants A edit on P -> upsert succeeds with current revision, no auto-start
desktop revokes A -> all later calls and active continuations rejected
```

### Task 9: Proxy script and Codex/Claude Code connection flow

**Files:** `src-tauri/src/proxy.rs`, early CLI dispatch in `main.rs`, `AiConnections.tsx`, setup snippet/wrapper generation, `docs/ai-connections.md`, `src-tauri/tests/proxy_output.rs`, client setup tests, sanitized evidence.

**Consumes:** real MCP endpoint, per-client tokens and grants. **Produces:** a bounded AI proxy script plus separate working connection instructions for the target clients, without modifying their configuration automatically.

- [ ] Verify current installed client versions and their documented HTTP auth fields before generating instructions. Keep sample tokens synthetic.
- [ ] Write failing proxy tests for stdout/stderr forwarding, attach-to-existing-run, cursor continuation, byte/duration caps, service failure, app offline, revoked credentials, paths with spaces, and interruption/closed pipe without stopping the managed service.
- [ ] Implement non-GUI proxy dispatch and the SDK client from the contract above. Generate the private wrapper/profile only on explicit local UI action; refuse silent overwrite. Keep generic shell execution and direct child spawning out of proxy mode.
- [ ] Exercise a real proxy subprocess against an isolated SDK test server and then the real manager. Assert one service run ID appears in the desktop, direct MCP reads, and proxy stdout. Kill the proxy subprocess and verify the service continues; explicitly stop the fixture through the manager afterward.
- [ ] Add copyable setup instructions and explain private token handling, local app availability, client reload, and revocation. Do not force terminal use for normal service management; connection instructions can target each client's settings UI or private config.
- [ ] Test snippet formatting and that token reveal is one-time. Do not put credentials in copied project configuration, app logs, or a URL.
- [ ] With authorized client setup, connect Codex and Claude Code separately to an isolated synthetic project and list services, start, query status/readiness, read stdout/stderr, upsert a harmless definition with edit grant, restart, and stop. Confirm UI reflects every action.
- [ ] From each target client's shell tool, execute the generated proxy to run a harmless registered fixture and read its stdout. Verify the AI can quote a unique synthetic log marker, then request later logs using the continuation cursor without starting a second run. Record this separately from direct MCP tool verification.
- [ ] Also test app offline, wrong token, revoked token, and restart reconnect. Record actual client surface/version and outcomes individually. A passing CLI test does not certify the desktop app surface.
- [ ] If user authentication or client settings changes cannot be performed, finish all unaffected setup/test work and clearly leave only those real-client gates pending. Never disable authentication to obtain a passing test.
- [ ] Commit setup documentation and sanitized evidence.

### Task 10: Packaging, performance measurement, and delivery

**Files:** release build configuration, `README.md`, `docs/verification.md`, milestone/task completion records; application bundle under the designated user-facing outputs directory.

**Consumes:** complete desktop/MCP flow. **Produces:** a locally runnable Apple Silicon `.app` bundle and an honest acceptance record.

- [ ] Run the complete regression gates listed below. Inspect the staged diff for unrelated identifiers, private paths, real tokens, real project logs, and generated assets.
- [ ] Build the release `.app`, launch it from Finder, and rerun Open Folder → Run → Logs → Restart → Stop with synthetic foreground and Compose fixtures.
- [ ] Measure release bundle size, time until the first interactive window, idle app+webview memory, and behavior during synthetic log floods. Separate managed service memory from manager/webview memory. Record OS, hardware, command, sample count, and raw values.
- [ ] Performance goals, not prior claims: interactive window within 2 seconds on the current development machine after OS first-launch checks; idle app+webview RSS target at or below 150 MiB; idle CPU average below 1% over 60 seconds; bounded flood memory consistent with configured retention. Record three startup samples and use the median.
- [ ] Investigate missed goals before adding optimization dependencies. If a goal remains unmet, report measured values and the limitation; do not label it passing without evidence.
- [ ] Confirm quit behavior, no managed fixture processes/containers left running, private state permissions, and no network listener beyond the loopback endpoint.
- [ ] Update README with build/run/setup instructions and known limits. Copy the deliverable into the outputs directory only after contamination checks. Do not publish, sign with a person's identity, upload, or enable auto-update.
- [ ] Commit source/docs only, verify final Git status, and report version, commit, proof, artifact location, pending acceptance gates, and measured performance.

## 6. Verification commands

Establish the scripts in Task 1 so these exact commands remain valid. Run from the repository root with the existing Rust toolchain on PATH. A shell used only for verification may prepend its existing `.cargo/bin`; do not edit the user's global shell configuration automatically.

```sh
rtk pnpm typecheck
rtk pnpm lint
rtk pnpm test --run
rtk pnpm build
rtk proxy cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
rtk proxy cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
rtk proxy cargo test --manifest-path src-tauri/Cargo.toml
rtk pnpm tauri build --bundles app
rtk git diff --check
rtk git status --short
```

Docker fixture tests must be explicit opt-in integration tests; they cannot silently use existing stacks. Run `rtk proxy cargo test --manifest-path src-tauri/Cargo.toml --test compose_lifecycle -- --ignored` only after confirming the disposable fixture target and runtime authorization. The default suite still tests validation/argument construction; it must report the real Docker test as skipped rather than disguise it as a pass.

Use deterministic Tokio time control for health thresholds and deadlines where possible; use bounded real waits for process group tests. Every integration test owns and cleans its synthetic resources, including on assertion failure.

## 7. Completion checklist

- [ ] Native application opens a code folder and remembers service settings after reopening.
- [ ] Users can add/edit command and Compose services without a terminal.
- [ ] Foreground processes and owned containers support run/stop/restart with verified ownership and cleanup.
- [ ] Concurrent requests cannot start duplicates or apply stale exit/health results.
- [ ] Live logs remain bounded, readable, run-scoped, and available to authorized AI clients.
- [ ] Health checks distinguish running from ready and remain cancellable.
- [ ] External config edits preserve user work and invalidate executable trust where required.
- [ ] Codex and Claude Code each pass their actual local connection/service-control checks, with target surface/version recorded.
- [ ] Proxy script forwards the same run's logs into actual AI shell-tool output; bounded capture, cursor continuation, revocation, and observer disconnection are verified without duplicated or implicitly stopped services.
- [ ] Unauthorized clients/projects cannot read logs or control services; revocation works.
- [ ] Closing/hiding and quitting behave as documented; failed cleanup is visible.
- [ ] Release bundle launches from Finder; measured performance and limits are recorded.
- [ ] Source/docs are committed cleanly; no secrets, unrelated context, or real private logs are included.

## 8. Stop conditions and deferred work

No routine product questions remain. Ask only if implementation needs credentials, modifies existing AI client settings, starts shared infrastructure, downloads/runs an unapproved container image, changes a security/data boundary, deletes user data, publishes a build, or needs to expand beyond macOS/local services. Present the exact target and completed local proof before requesting that approval.

Deferred: Windows/Linux/Intel support, embedded AI chat, SSH/remote hosts, deployment, multi-user accounts, dependency graphs/Start All, automatic dependency installation, terminal input/PTY support, auto-restart, automatic recovery/adoption after a crash, persistent searchable log archives, telemetry, public distribution, notarization, and auto-updates. Foreground noninteractive processes are supported. Connect stdin to null so input receives EOF rather than waiting on an unavailable input channel; explain missing-TTY errors. Arbitrary commands can still wait internally, so keep Stop usable and do not claim universal detection of interactive programs.

Expected known limits: same-user code can read local files the user can read; this app cannot protect secrets from arbitrary already-authorized same-user code. A trusted shell command can affect resources beyond its working directory. Process groups cannot guarantee control over deliberately daemonized descendants or app SIGKILL/OS crashes. These limits must be in release documentation and must not be hidden by UI wording.

## 9. Planning evidence and official references

Read-only environment inspection on 2026-10-16 found macOS Darwin arm64, Node `v26.8.1`, pnpm `10.33.2`, Xcode developer directory available, Rust `1.97.0`, and Cargo `1.97.0`. Rust binaries exist under the normal cargo bin directory but were not discoverable from this shell's PATH; absolute executable checks passed. Docker executable exists; daemon availability was not checked and no containers were started. No dependencies were installed and no build was run during planning.

The plan chooses a single local HTTP endpoint based on documented transport/auth support, not on a completed client connection. The following official sources were opened during planning:

- [OpenAI local MCP configuration](https://developers.openai.com/codex/mcp/): Streamable HTTP, bearer-token environment variables, headers, and local configuration. The page currently redirects to the official ChatGPT Learn MCP documentation.
- [Claude Code MCP reference](https://code.claude.com/docs/en/mcp): HTTP server configuration and Authorization headers. This establishes Claude Code configuration support, not Claude Desktop/web compatibility.
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/): platform build prerequisites to recheck in Task 1.
- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk): use the maintained SDK and current examples instead of writing protocol framing.

## 10. Plan review

The plan maps every agreed flow to a task: Open Folder/config persistence (1–2), editable commands and trust (2–3), lifecycle/logs/readiness (3–5), Compose (6), complete desktop navigation (7), shared AI control (8), proxy-script log forwarding and real Codex/Claude Code connections (9), and release/performance proof (10).

Fixed choices are recorded rather than left as unanswered product questions. Deployment, public signing, unrelated projects, and remote execution stay outside scope. Implementation starts at Task 1 only after the user requests execution; this planning request does not install dependencies or modify external client settings.
