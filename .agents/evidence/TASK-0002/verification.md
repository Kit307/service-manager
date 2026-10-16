# TASK-0002 evidence — 2026-10-16

## Source and runtime

Version 0.2.0 adds typed owned Docker Compose lifecycle, immutable private snapshots, local-daemon pinning, container/resource ownership, rejection of unsupported shared/scaled configurations, whole-group runner cleanup, Docker/HTTP health, and retained-volume restarts. Existing command lifecycle remains shared with MCP/proxy.

Tauri emits change notifications at 100 ms cadence. The renderer fetches bounded pages and caps its own retained text. Disk config refresh runs every two seconds. Long newline-free UTF-8 records are split with an explicit truncation marker. HTTP startup health exposes an error at 30 seconds and recovers without stopping the process.

## Executed gates

- `pnpm test --run`: 6 passed.
- `pnpm build`: TypeScript and production frontend build passed.
- `pnpm lint`: passed.
- `cargo test --no-default-features`: 16 passed; live Docker test intentionally ignored in default suite.
- `cargo test --no-default-features --test compose -- --ignored`: live local fixture passed, including unlabeled-volume refusal, stop during preflight, duplicate start, source-edit-safe Stop, preserved volume data and shutdown.
- `cargo test --no-default-features --test mcp_proxy`: official SDK and actual proxy subprocess passed.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets -- -D warnings`: passed.
- `pnpm tauri build --bundles app`: arm64 release built; final archive checks recorded below.

Commands above used RTK and explicit Rust manifest paths. Loopback and Docker tests required local sandbox elevation; no shared stack was changed. The fixture uses the already installed Alpine image without image pulls. Generated snapshots/credentials use private application state and are excluded from source control.

## Native UI

An isolated release QA identity exercised Open Folder → Cancel → Open Folder → Add → Save → Trust → Run → stdout marker → Restart (new PID) → Stop → Reopen. Native accessibility showed textual status and real output. Both themes were visually inspected. Screenshots contain only the synthetic workspace.

A separate 900 × 600 release shell was inspected. Native Quit exposed a default macOS-menu bypass: the predefined item uses `NSApp terminate:`. It was replaced with a custom Cmd+Q item routed through the controlled exit path. The prompt is now parented to the main window rather than appearing as a detached system alert. A native debug recheck showed the Quit sheet, verified Cancel, and confirmed Stop services & quit. Temporary trace code was removed.

Independent focused source review found/fixed early terminal publication, surviving Compose plugin children, unverified volume/network reuse and replica-status mismatch. Final reviewer source recheck reported no additional P1/P2 within that scope.

## Installed clients

Codex CLI 0.153.4, using per-invocation MCP settings and a temporary project, passed direct MCP log reading. A second invocation passed service control and shell-proxy output with the synthetic marker. A first proxy attempt was sandbox-network-blocked; the isolated test invocation enabled network for its loopback connection and then passed. No personal Codex settings were changed.

Claude Code 2.1.181 was installed but had no usable model authentication. The user explicitly selected preparing configuration first. Private copyable settings are implemented and formatting/reveal/hide are tested. Actual Claude model control is deferred until login. No Claude settings were changed.

## Performance limits

Final release app: 13.4752 MiB; ZIP: 5.6713 MiB; Mach-O arm64. ZIP integrity, executable Unix mode and binary SHA256 match were checked. No private state, credentials, environment files or project config are included. The binary has the toolchain's ad-hoc signature; it has no Developer ID/notarization. Native screenshots prove a real webview, but automation wall time includes tool transport/OS launch delays and is not a reliable intrinsic startup benchmark. Observed first-window upper bounds varied (1.222 s, 2.789 s and 3.921 s across different isolated runs); these are not three controlled samples, so no <=2 s median claim is made.

An interactive QA snapshot showed app RSS 105,968 KiB and three contemporaneous WebKit processes totaling 106,256 KiB (about 207.3 MiB summed RSS). It is not an idle benchmark or precise shared-memory attribution, and does not establish the 150 MiB goal. CPU values from that snapshot included interaction, so no <1% idle claim is made. A later isolated empty-state release measurement ran for 62.14 seconds with no managed service, tracking the app and three newly created WebKit processes: CPU average 0.322%, summed RSS 120.08–157.81 MiB (median 133.70 MiB); app-only final RSS 83.39 MiB. Hardware: Apple M5, 24 GiB, macOS 27.0.1 arm64. Sampling used process CPU-time deltas and RSS once per second after a 5-second settling delay; a release build ran concurrently. WebKit attribution used newly created processes after launch; summed RSS can double-count shared pages. Idle CPU was below the 1% goal, but peak summed RSS slightly exceeded the 150 MiB goal. A controlled three-run intrinsic startup benchmark and live full-app flood footprint remain unverified; the core 20 MiB flood retention test passed.

## Boundaries

Unsigned local build. No publishing, notarization, deployment, remote Docker, or changes to personal client settings. Original exhaustive hostile-input and every-client-surface acceptance matrix is not certified. Deliberately daemonized descendants and abrupt app/OS termination remain outside guaranteed recovery.

## Archive receipt

- ZIP SHA256: `5803694d2d1decfaff0a613284f207c94e0351ec4a8c511b2e08449c543fbcd8`
- Executable SHA256: `ab5430e4049617887873c26fd6d85bfd616b121fe93ebe92eee5342f8f7bb99f`
- Existing 0.1.0 output was preserved; delivery is a new 0.2.0 archive.
