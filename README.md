# Service Manager

A desktop app for running project services without typing commands in a terminal.

Status: the first working desktop UI build is implemented. Command services, project configuration, live logs, loopback health checks, and authenticated MCP/proxy access are connected to the Rust manager. Docker Compose lifecycle and the complete delivery acceptance matrix remain unfinished; this is a local development build.

## Selected stack

- Tauri for the desktop application shell and native webview.
- React and TypeScript for the user interface.
- Rust for shared service lifecycle, configuration, log collection, and MCP integration.

## Agreed workflow

1. Open a project folder.
2. Load its saved service configuration, or add a service and its run command.
3. Run, stop, or restart services and view live logs.
4. Reopen the folder to restore its saved configuration.
5. Let Codex and Claude use MCP to manage registered services and read their logs.
6. Let AI shell tools call a proxy script that starts a registered service through the same manager and forwards its logs to the tool output.

Each project can contain multiple services with different working directories and commands, including Go, Node.js, and Docker Compose.

See [the design](docs/superpowers/specs/2026-10-16-service-manager-design.md) for behavior and implementation boundaries.

The [implementation plan](docs/superpowers/plans/2026-10-16-service-manager-plan.md) includes a Thai overview, fixed first-release choices, architecture, service, MCP and proxy-script contracts, ten sequential implementation tasks, and delivery acceptance checks. It describes the complete target, not a claim that every task is finished.

## Use the desktop app

Open Service Manager, choose Open Folder, and add a command service. Save the definition, review it on Run, and approve its exact configuration before first execution. Settings and commands persist in `.service-manager/project.json`. Existing malformed files are preserved and reported as errors.

Run supports foreground noninteractive commands such as `pnpm dev` and `go run ./cmd/api`. Stop signals the owned process group, escalates after five seconds, and waits for cleanup. Opening a folder does not execute anything. Running and Ready remain different states; configure a loopback health check to establish readiness.

Closing the window hides it. Use the menu/tray or Dock to reopen it. Quit asks to stop active services first and blocks new starts while cleanup is in progress. Commands run with your normal user permissions; the application is not a sandbox for untrusted repositories. Deliberately detached children and abrupt OS/app termination are outside guaranteed cleanup.

## Development

Requires macOS 13 or later on Apple Silicon, Rust/Cargo on PATH, Xcode tools, Node.js 22.16 or later, and pnpm 10.33.2. Dependencies are pinned in `pnpm-lock.yaml` and `src-tauri/Cargo.lock`.

```sh
rtk pnpm install --frozen-lockfile
rtk pnpm tauri dev
rtk pnpm test --run
rtk pnpm typecheck
rtk pnpm lint
rtk proxy cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
rtk proxy cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
rtk pnpm tauri build --bundles app
```

The loopback integration tests need permission to bind disposable local ports. A sandbox denial is not an application test failure. Tests use temporary roots and synthetic commands, never existing user services or Docker stacks.

For isolated manual testing, set `SERVICE_MANAGER_DATA_DIR` to a new directory under the workspace before launching the development app. This does not change the normal app-data location for end users. A normal browser can inspect the empty React shell; native folder/process controls require the desktop app.

## AI access

See [AI connection instructions](docs/ai-connections.md). The UI and both AI access paths share one manager. SDK/proxy integration is tested against a real isolated process; the user's actual Codex and Claude configurations have not been modified or certified.

## Known remaining work

- Typed Docker Compose definitions can be loaded/edited but Run explicitly returns `COMPOSE_PENDING` without starting Docker. The complete owned-container lifecycle is a subsequent task.
- Full real-client Codex/Claude setup, a complete native UI lifecycle smoke run, and the complete security/performance acceptance matrix remain pending.
- Current UI refreshes status/logs by bounded local polling. The planned batched Tauri event transport and broader live-load benchmark are not implemented.
- First-build health checks report checking/healthy/unhealthy; the planned separate 30-second startup-health diagnostic remains pending.
- No public signing, notarization, publishing, auto-update, SSH, or deployment is included.
