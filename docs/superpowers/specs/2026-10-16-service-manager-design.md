# Service Manager Design

## Objective

Provide a desktop interface for opening a code folder and managing its development services. Codex and Claude must be able to control the same services and read the same logs through MCP.

This document records the agreed product flow and selected stack. Client-specific connection setup remains an implementation planning decision; compatibility has not yet been tested.

The complete [implementation plan](../plans/2026-10-16-service-manager-plan.md) records the fixed first-release choices, module contracts, sequential tasks, security boundaries, and acceptance checks. It selects macOS Apple Silicon first, project-local JSON, and an authenticated loopback Streamable HTTP MCP endpoint shared by the desktop application and local Codex/Claude Code clients.

## Selected stack and ownership

Use Tauri for the desktop shell, React with TypeScript for the interface, and Rust for service management and MCP integration. Tauri uses the platform webview, supporting the goal of a lightweight desktop application. Actual application size, startup time, and memory usage must be measured before making performance claims.

React renders project configuration, service status, controls, and logs. It calls a narrow set of validated Tauri commands and receives status and log events. It must not own service processes or execute shell commands directly.

The Rust manager owns all service state and lifecycle invariants. The MCP interface calls the same Rust manager as the Tauri commands. AI connections must not spawn independent service managers. No Node.js runtime is required for the application's own backend; managed project services may still require Node.js or other tools installed on the machine.

## Desktop workflow

The user selects Open Folder. The app loads the project's saved configuration. If none exists, it presents an empty state with Add Service. The app must not execute commands merely because a folder was opened.

A service has a stable identifier, a display name, a working directory relative to the project root, and a run command. Optional settings include environment variable names or references and a health-check URL. Secret values must not be written into shareable project configuration.

The user can add and edit services, then run, stop, or restart them. Changes persist per project. The interface displays projects and services on the left, status and controls in the center, and live logs below.

## Shared process ownership

One local service manager owns process lifecycle, status, configuration writes, and log collection. Both the desktop interface and MCP call this manager. MCP connections must not create independent managers or duplicate service processes.

Concurrent start requests for the same service must produce at most one managed process. Configuration writes must be validated and atomic. A command change applies to the next run rather than silently restarting an active service.

The manager distinguishes stopped, starting, running, ready, stopping, and failed states. A running process is only marked ready when its configured health check succeeds. Without a health check, the app reports running without claiming readiness.

Stop targets the owned service and its child processes, never unrelated processes using a matching name or port. Restart waits for stop to finish. Docker Compose services require explicit lifecycle handling because a command exiting does not necessarily mean its containers stopped.

The initial lifecycle policy is to keep management available while the window is hidden. Quitting the manager must offer stopping active services or cancelling quit; leaving unmanaged processes behind is not an implicit default.

## Logs

Capture stdout and stderr with timestamps, service identifiers, and run identifiers. The UI receives live updates. Keep bounded log history to prevent unbounded memory or disk use.

MCP exposes bounded reads with a cursor so AI clients can request recent output and fetch subsequent lines without rereading everything. Logs are untrusted service output, never instructions to execute commands. Log access may reveal application data or secrets already printed by the service; it must remain scoped to authorized projects.

## MCP integration

Expose tools to list authorized projects and services, read and update service configuration, start, stop, and restart registered services, query status, and read logs.

Each operation uses explicit project and service identifiers. Switching the selected UI folder must not redirect an in-flight AI request to a different project. Restrict access to projects explicitly opened and authorized locally.

MCP must not expose an arbitrary standalone shell-execution tool. Editing a service command still grants code-execution capability, so the connection must be local and authenticated. Imported executable configuration requires local trust before first execution. AI changes require an explicit local project-specific edit grant, which can authorize subsequent MCP edits without a prompt per edit; MCP cannot grant this permission to itself. External file changes still invalidate affected executable trust. Do not automatically execute configuration supplied by an untrusted repository.

Codex and Claude connect to the same manager using the MCP transport supported by their target clients. Provide client setup instructions only after confirming the chosen transport works with each client. No embedded AI chat or model API key is required in the desktop app.

## Initial scope

- Open Folder and persistent service configuration.
- Add and edit services and their run commands.
- Run, stop, restart, status, and live logs.
- MCP access for Codex and Claude to the same service manager.
- Local development services only; opening a folder never grants deployment or remote-server authority.

## Verification required before delivery

- Save configuration, close and reopen the project, and verify it reloads correctly.
- Reject malformed configuration and invalid working directories without starting processes or losing saved settings.
- Exercise start, duplicate concurrent start, stop, restart, failed commands, health checks, and child-process cleanup using isolated fixtures.
- Verify Docker Compose status and shutdown against owned fixture containers.
- Verify stdout and stderr capture, run separation, bounded history, and incremental MCP reads.
- Confirm UI and AI operations share state, including project switching during an AI request.
- Confirm unauthorized clients and projects cannot control processes or read logs.
- Perform actual connection and service-control smoke checks from both Codex and Claude before claiming client compatibility.
