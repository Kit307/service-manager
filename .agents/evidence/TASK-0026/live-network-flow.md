# Live Network flow verification

Date: 2026-10-28. Scope: isolated synthetic transport and automated checks; native UI acceptance remains incomplete.

## Automated checks

- `rtk pnpm test --run --maxWorkers=1`: 17 files, 140 passed.
- `rtk pnpm typecheck`, `rtk pnpm lint`, `rtk pnpm build`: exit 0.
- `rtk proxy /Users/krit/.cargo/bin/cargo test --all-targets` from `src-tauri`, with local socket permission: 105 passed, 6 ignored, 0 failed. Initial sandbox run stopped at the library suite with 37 passed, 6 failed and 1 ignored due to socket permission denial; the permitted rerun passed all targets.
- `rtk proxy /Users/krit/.cargo/bin/cargo test --test vpn_docker native_vpn_routes_keep_commands_on_host_and_share_gateways -- --ignored --nocapture`: passed, printed `fixtureCleanupComplete`. The actual synthetic OpenVPN route remained alive while a known request added exactly 54 TX and 70 RX payload bytes; counters did not change over the following 200 ms idle sample. Existing proof also checked shared gateways, native workers, stop cleanup and fail-closed gateway loss.
- `rtk git diff --check`: exit 0.

## Isolated app attempt and limitations

Before and after, production MCP 127.0.0.1:47831 was held by PID 66531, the installed Service Manager executable, with the same process start time. Existing Vite 127.0.0.1:1420 was held by PID 47988, the repository Vite process, with the same start time; it was not started or stopped for this test. Port 47832 was free before and after.

A fresh empty temporary data directory was used with `SERVICE_MANAGER_MCP_PORT=47832` and `src-tauri/tauri.test.conf.json`. The source launch and the documented `pnpm test-app` harness were each tried with `--config '{"build":{"beforeDevCommand":""}}' --no-watch` to reuse the existing Vite rather than spawn another. Both compiled/ran `target/debug/service-manager`, then exited 0 without an error message. No data was written to the temporary directory and no 47832 listener appeared. The empty directory was removed. The source has a single-instance plugin, but the reason for exit was not established.

No isolated native UI was available in the initial attempts. Production service run IDs and configuration were not inspected, so only listener process identity is established. The pre-existing 1420 listener remains open by design. No app bundle, install, deployment or production service control was performed.

## Unique-identity retry

The earlier Test identifier had an existing singleton holder, PID 39400; it was not used or stopped. A new inline config overrode the identifier to `dev.servicemanager.desktop.test.task0026`, product and title to `Service Manager Test TASK-0026`, and disabled `beforeDevCommand`; `pnpm test-app` retained MCP port 47832 and used a new temporary data directory. PID 45178 owned the unique singleton socket and its new state file contained zero projects. This proves isolated startup and empty persisted project state, not a native UI or MCP interaction; 47832 did not listen because MCP was not enabled. Native UI automation returned `Invalid app` for the unique identifier, so the native window could not be targeted safely.

Separately, a browser opened the existing Vite server at 1420 without a native bridge. In that browser preview, Live Network showed `Unavailable`, dashes for totals, and `No VPN routes configured`. The system, dark and light appearance controls were selected, and light and system-dark screenshots were inspected. The opacity slider displayed 55%, then keyboard Home and End selected 30% and 100%, and it was restored to 55%; appearance was restored to system. At 900×600, the summary wrapped and the route headings were clipped inside the horizontally scrollable container; a larger browser viewport was also viewed. This preview does not prove native transparency, scrolling interaction, configured-route selection/focus, neutral route lines, active telemetry, reduced motion or private-data handling. It showed no private data in its empty state. The browser viewport was reset and the tab closed.

Ctrl-C stopped only the launched session; PID 45178 disappeared, its unowned unique socket and temporary data directory were removed. Port 47832 remained closed. Existing Test singleton PID 39400, production singleton/47831 PID 66531, and Vite 1420 PID 47988 remained present. Native UI acceptance remains incomplete.

## Native empty-state inspection

A subsequent fresh isolated launch used the same unique identifier and a new temporary data directory whose state contained zero projects. The controller focused PID 47772 and captured its native window at exactly 900×600 in one atomic focus-and-capture command. The capture showed the title `Service Manager Test TASK-0026`, Live Network selected, 0 observable routes, 0 pulsing directions, dashes for open connections and TX/RX, `No active routes`, `No VPN routes configured`, 30% opacity, and Liquid Glass cards. Graph content was clipped within its own horizontal container rather than overflowing the page.

Later cleanup inspection found PID 47772, its temporary data directory and singleton socket absent, with port 47832 closed. No signal or removal was necessary. Production and Vite process identities had changed independently by then and were not touched; unchanged identity is not claimed for this later interval.

This empty-state capture does not prove configured-route behavior, animation, reduced motion, forced colors, route keyboard selection, a larger native window, or live telemetry in the UI. Native acceptance remains partial.
