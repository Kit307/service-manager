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

No isolated native UI was available. Zero-project identity, Live Network rendering, light/dark/system, 30/55/100% opacity, window sizes, keyboard interaction, reduced motion, overflow, idle/unsupported states, and privacy inspection were not exercised. Production service run IDs and configuration were not inspected, so only listener process identity is established. The pre-existing 1420 listener remains open by design. No app bundle, install, deployment or production service control was performed.
