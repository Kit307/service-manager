# Service Manager

A desktop app for running project services without typing commands in a terminal.

Version 0.6.1 implements the local desktop workflow: command and owned Docker Compose services, project configuration, live logs, bounded run history, dependency-aware group actions, port diagnostics, loopback health checks, and authenticated MCP/proxy access share one Rust manager. This is a local macOS development release without Developer ID signing or notarization.

## Selected stack

- Tauri for the desktop application shell and native webview.
- React and TypeScript for the user interface.
- Rust for shared service lifecycle, configuration, log collection, and MCP integration.

## Agreed workflow

1. Choose Add service and select its working folder. Existing services in that folder appear automatically.
2. Add a command or Compose definition.
3. Organize services directly into virtual groups, or leave them ungrouped.
4. Open a group to Run all, Stop all or Restart all; select any service for live logs. Groups and disclosure state survive reopening the app.
5. Let Codex and Claude use MCP to manage registered services and read their logs.
6. Let AI shell tools call a proxy script that starts a registered service through the same manager and forwards its logs to the tool output.

Services may use different working folders and commands, including Go, Node.js, and Docker Compose. The UI uses Group → Service directly; existing project identities remain internally for configuration, process ownership and MCP compatibility. Existing service definitions, trust records and working roots are preserved.

See [the design](docs/superpowers/specs/2026-10-16-service-manager-design.md) for behavior and implementation boundaries.

The [implementation plan](docs/superpowers/plans/2026-10-16-service-manager-plan.md) includes a Thai overview, fixed first-release choices, architecture, service, MCP and proxy-script contracts, ten sequential implementation tasks, and delivery acceptance checks. It describes the complete target, not a claim that every task is finished.

## Use the desktop app

AI Connections can bootstrap an empty workspace: select an allowed parent folder when creating a client profile. MCP can then discover its allowed roots, create new empty project folders, or open existing code folders itself. Its configured services appear directly in the same app; one token controls all registered services and nothing runs automatically. See [AI connections](docs/ai-connections.md).

The macOS title strip is replaced by content with native window controls retained. Drag the top area to move the window; resize its edges down to 900 × 600. Dock minimization remains disabled and startup uses the normal window size. Open Service Manager, choose Add service, select its working folder, and add a command or Docker Compose service. Save the definition, review it on Run, and approve its exact configuration before first execution. Settings and commands persist in `.service-manager/project.json`. Existing malformed files are preserved and reported as errors.

Run supports foreground noninteractive commands such as `pnpm dev` and `go run ./cmd/api`. Stop signals the owned process group, escalates after five seconds, and waits for cleanup. Opening a folder does not execute anything. Running and Ready remain different states; configure a loopback health check to establish readiness.

Use New group or a group's settings to create, rename or remove a group and select its services. Selecting a service moves it from its previous group. Removing a group keeps its services under Ungrouped and never touches code or running processes. Drag a service onto a group or the Ungrouped area to move its membership without changing files or interrupting its run. Settings remains available for keyboard use. Grouping does not limit token access.

Run all reuses active services and starts dependencies before their dependents. Stop all uses reverse dependency order; Restart all stops and then starts the selection. All services applies to the entire workspace. Required trust reviews finish before execution. A dependency must become Ready before its dependent starts; failures stop the sequence. Cancel sequence prevents later actions and leaves services already started running. Dependencies outside the selection must already be Ready. See [service operations](docs/service-operations.md) for configuration, limits and recovery. Remove a stopped service through its Settings; this removes its definition and group membership, preserving source files.

Closing the window hides it. Use the menu/tray or Dock to reopen it. Quit asks to stop active services first and blocks new starts while cleanup is in progress. Commands run with your normal user permissions; the application is not a sandbox for untrusted repositories. Deliberately detached children and abrupt OS/app termination are outside guaranteed cleanup.

The macOS window uses the Rust-only [tauri-plugin-liquid-glass 0.1.6](https://github.com/hkandala/tauri-plugin-liquid-glass) with the standard native Clear variant on macOS 26+, and an NSVisualEffect blur fallback on older macOS. Text stays opaque and logs use a darker surface for contrast. The plugin uses private APIs, so future macOS compatibility and App Store suitability are not guaranteed. See [the visual approach and verification limits](docs/service-operations.md#liquid-glass). The sidebar Background opacity slider persists a 30–100% background setting (default 55%, steps of 5); text/log opacity is unchanged. Reduced-transparency or high-contrast preferences force opaque backgrounds. Buttons, cards and the tab bar use raised clear-glass capsule rims, highlights and soft shadows.

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

Every valid MCP/proxy token, including existing tokens, now grants read/control/command-edit access to all registered service folders and folders registered later. No per-project selection is required. MCP start/restart authorizes the current registered definition through the existing revision/trust mechanism; the manager still revalidates config and owns all process control. Keep tokens private: they authorize execution with your normal user permissions. Revocation, disabled connections and Origin rejection remain enforced. Allowed parent folders still bound filesystem creation/opening; the token does not scan arbitrary disk folders.

An installable [Service Manager skill](docs/service-manager-skill.md) is included under `skills/service-manager` for Codex and Claude Code. It teaches MCP control, safe config edits and proxy log continuation; credentials and grants remain separate.

See [AI connection instructions](docs/ai-connections.md). The UI and both AI access paths share one manager. The installed Codex CLI passed isolated MCP and shell-proxy checks. Claude Code setup is prepared; its real-client check is deferred until login, as requested. Personal client settings are never modified automatically. New profiles show copyable private configuration snippets in AI Connections.

## Docker Compose

Compose requires a running local Unix-socket Docker context and locally available images. The app pins the selected daemon, resolves configuration privately, labels containers/networks/volumes with a private ownership identity, and refuses pre-existing resources it cannot verify. Each run uses a private immutable snapshot, so editing the source cannot redirect Stop. The Docker CLI and Compose plugin process group must finish before Stop succeeds. Stop uses `compose stop --timeout 5`; it never invokes `down`, prune, or volume removal.

The supported Compose subset is one foreground long-running container per service. Scaling overrides, explicit container names, profiles, external resources, Compose secrets/configs, and remote Docker contexts are rejected with a readable error. Use a command service for existing custom workflows; its command still has normal user permissions. The app does not install tools, build images, or pull missing images automatically.

## Service views and log levels

Output, Settings, History and Diagnostics share the service heading, live status and lifecycle controls. Settings scrolls within the content pane. Switching tabs preserves log filters/follow and unsaved settings; Cancel discards the draft. Saving retains the revision captured when editing began, so background refresh cannot silently overwrite external changes.

Clicking a service in the sidebar opens its full Output view. Open in log workspace or a pane picker provides the separate comparison view. The Log workspace keeps multiple service tabs across any number of panes, with no fixed two-pane or tab-count cap. Add a pane, drag tabs to reorder/move them, or use the keyboard controls. Each service has one tab with its own session search, stream filter and follow setting. Only active tabs in visible panes observe logs; hidden/offscreen observers pause and release their local output. Returning reads the manager's bounded output and reports retention gaps. Closing a tab never stops its service. Only layout and explicit service references persist between app launches, not log content or filter/search state. See [log workspace controls](docs/service-operations.md#log-workspace-v061).

Stream labels are `stdout`, `stderr` and `system`; stderr is not itself a severity level. Only an explicit recognized console prefix or JSON string `level` colors a line: errors red and warnings amber. Informational and unknown formats stay neutral, regardless of stream. Message content, raw stream identity and MCP/proxy output are preserved. Numeric levels and arbitrary logger formats are not inferred.

## Runtime limits

Output is retained in memory with per-service/global bounds. The desktop receives change events in 100 ms batches and fetches bounded pages; it does not continually poll logs while idle. Configuration changes on disk are checked every two seconds. Health checks expose a diagnostic after 30 seconds without success and continue checking for recovery.

Run history is stored privately with at most 100 runs overall, 20 per service, and 64 KiB/500 log records per completed run. It is a bounded diagnostic archive, not a full log backup. Redaction is best effort; review copied diagnostic context before sharing. No public signing, notarization, publishing, auto-update, SSH, remote control, automatic restart, or deployment is included. The original plan's broader acceptance matrix is tracked honestly in [verification](docs/verification.md), including measured limits and untested surfaces.

Service rows use captured pointer movement for group changes, avoiding native WebKit HTML drag interception. Drag onto a group, including a collapsed group, or onto Ungrouped to remove membership. Escape, pointer cancellation and drops outside these targets leave membership unchanged. Group Settings remains the keyboard-accessible alternative.
