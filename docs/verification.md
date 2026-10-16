# Desktop UI build verification

## Scope

This delivery implements the agreed desktop interface plus a working command-service manager and local MCP/proxy path. It does not complete every task in the full implementation plan. Docker Compose lifecycle, actual personal AI client setup, complete native lifecycle interaction, and the full release acceptance matrix remain open.

## Evidence collected on 2026-10-16

- React shell renders in a native Tauri macOS `.app`, not only a browser. Its empty state and native folder picker were inspected through the native accessibility interface, and the initial empty-state screenshot was saved as a user-facing output.
- Native inspection stopped when the app's UI state changed during interaction. No unrelated project configuration or service was run for the smoke test. Further runtime proof uses isolated temporary fixtures.
- React behavior tests exercise initial launch, inert/plain-text log rendering and filtering, preserving unsaved config drafts and their original revision during polling, hiding removal in a new-service form, and preserving logs when reselecting the same service.
- Rust tests exercise path/schema/duplicate validation; persistent config reopening; stale write preservation; symlink rejection; strict command/Compose fields; trust binding to the reviewed revision; actual process start and duplicate-start prevention; real stdout collection and stop; bounded log/cursor behavior; authenticated MCP and proxy with shared run identity; authorization/revocation; and credential file permissions.
- Independent reviewer identified a trust race, shutdown admission race, and log-reselection bug. Each responsible layer was corrected; focused source recheck confirmed all three fixes. Trust and log-reselection tests were observed failing before the fixes and passing afterward.
- A debug application bundle built and launched successfully. It measured 78.93 MiB, which is a debug build size rather than a release performance claim. Release size and final commands are recorded in the task evidence after build completion.
- The final release bundle built successfully at 13.32 MiB and was identified as Mach-O arm64. It contains no private application state or credential files. Final gates passed: 5 React tests, 13 Rust tests, typecheck, ESLint, Clippy with warnings denied, and Rust formatting. The release executable has not undergone the full native lifecycle/performance smoke matrix; the earlier native launch used the debug bundle.
- Node 26's experimental global storage shadowed jsdom's storage in the test worker. A differential run with the native Node storage feature disabled passed. Vitest workers now disable that feature explicitly; the application's browser storage behavior was not replaced with fake production state.

## Safety boundaries

Tests use synthetic folders and commands. No real Docker container was started, no production endpoint was called, no personal AI settings were modified, and no artifact was published. Private app state, generated proxy credentials, logs, dependency trees, and native build outputs are excluded from Git.

Foreground Unix process groups are the supported lifecycle boundary. Deliberately daemonized descendants and catastrophic app/OS termination are not guaranteed to be recovered. The app reports unclean prior sessions instead of guessing ownership from a stored PID.

## Remaining acceptance work

- Actual Codex app/CLI and Claude Code version-specific connection checks.
- Typed Compose ownership, reconciliation, data preservation, and live container cleanup.
- Full native Open Folder → Add → Trust → Run → Logs → Restart → Stop → Reopen smoke flow, keyboard navigation, compact window and both theme checks.
- Wider invalid/hostile input and live-load matrices, proxy interruption/closed-pipe stress, and complete health failure/recovery matrices.
- Release startup, idle CPU, app+webview memory, and high-output memory measurements. Do not infer performance from Tauri selection or bundle size alone.
