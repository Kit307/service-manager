# Live Network Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an evidence-based Live Network overview whose directional dots appear only when Service Manager-owned native VPN relays move payload bytes.

**Architecture:** Native relay tasks maintain per-route atomic payload counters and active-tunnel counts. The manager exposes one non-persistent desktop-only snapshot, while a React hook polls without overlap and derives short-lived TX/RX pulses from positive deltas. The page joins live telemetry to existing service and VPN configuration; unsupported traffic is labelled unobservable.

**Tech Stack:** Rust, Tokio, Tauri 2, React 19, TypeScript, CSS/SVG, Vitest, Testing Library.

**Spec:** `docs/superpowers/specs/2026-10-27-live-network-flow-design.md`

## Global Constraints

- Count only payload bytes successfully written after CONNECT succeeds; never count handshake bytes.
- Never infer activity from service state, VPN state, tunnel count, configured routes, timestamps, or color.
- Direct native, legacy Docker VPN, DNS, UDP, and external-process traffic remain `Not observable`.
- Telemetry is memory-only, desktop-only, bounded to current native relay routes, and contains no credentials or logs.
- Poll only while the Live Network page is mounted; interval is 1 second and requests never overlap.
- Initial, missing, failed, decreased-counter, and changed-run samples establish a baseline without pulses.
- TX and RX pulses expire independently after 1.5 seconds; reduced motion hides movement but not textual state.
- Keep the restored Liquid Glass theme, 900×600 minimum, visible focus, forced colors, and no new dependency.
- Do not build/install a Tauri bundle, create a DMG, touch production services, push, publish, or deploy.

## Review Focus

- Partial writes and write failures must count only bytes actually forwarded; Task 1 tests both directions before close.
- An established but idle tunnel must report one connection and zero byte deltas; Task 1 tests it.
- Stale cleanup from an old run must not remove a replacement run; Task 2 tests run-key comparison.
- Slow/failed polling and unmount must not overlap, animate stale data, or update after cleanup; Task 3 uses deferred promises and fake timers.
- Identical destinations across projects/routes must remain distinct and keyboard-selectable; Task 4 tests stable route identity.

---

### Task 1: Native relay payload counters

**Files:**
- Modify: `src-tauri/src/vpn_relay.rs`
- Modify: `src-tauri/tests/vpn_relay.rs`

**Interfaces:**
- Consumes: existing `RelayTarget` and `VpnRelaySet::start(Vec<RelayTarget>)`.
- Produces: `RelayTrafficSnapshot { route: VpnRoute, tx_bytes: u64, rx_bytes: u64, active_connections: u64 }` and `VpnRelaySet::traffic() -> Vec<RelayTrafficSnapshot>`.

- [ ] **Step 1: Write failing relay telemetry tests**

Add tests that sample before tunnel close and assert: idle CONNECT has one active connection with zero counters; known service-to-remote bytes increment TX; known remote-to-service bytes increment RX; simultaneous routes stay independent; rejected CONNECT stays zero; closing returns active count to zero.

- [ ] **Step 2: Run RED**

Run `rtk cargo test --no-default-features --test vpn_relay` from `src-tauri`.

Expected: compile/test failure because telemetry types and `traffic()` do not exist.

- [ ] **Step 3: Implement minimal counters**

Use one `Arc` of relaxed atomics per route. Wrap the two payload writers so each successful `poll_write` increments only its direction by the returned byte count. Use an RAII established-tunnel guard after CONNECT acceptance. Preserve current half-close/error behavior and `VpnRelaySet` drop cleanup.

- [ ] **Step 4: Run GREEN and regression**

Run `rtk cargo test --no-default-features --test vpn_relay` and `rtk cargo test --no-default-features --lib` from `src-tauri`.

- [ ] **Step 5: Commit**

Commit `feat(network): measure native relay traffic` with only Task 1 files.

---

### Task 2: Current-run telemetry registry and desktop command

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src-tauri/src/manager.rs`
- Modify: `src-tauri/src/desktop.rs`
- Test: `src-tauri/src/manager.rs` unit tests or a focused new integration test under `src-tauri/tests/`

**Interfaces:**
- Consumes: `VpnRelaySet::traffic()` from Task 1.
- Produces: serializable camel-case `NetworkTraffic`, `Manager::network_traffic() -> Vec<NetworkTraffic>`, and Tauri command `network_traffic`.

- [ ] **Step 1: Write failing registry/lifecycle tests**

Cover current-run identity, sorted bounded snapshot output, explicit removal, stale old-run removal not affecting replacement, and no registration after setup/spawn failure. Add a command serialization assertion for camel-case fields.

- [ ] **Step 2: Run RED**

Run the focused Rust tests. Expected: failure because the registry, DTO, and command do not exist.

- [ ] **Step 3: Implement the run-scoped registry**

Add a short-held standard mutex registry to `Manager`. Register traffic only after native process spawn succeeds. A synchronous run guard removes only a matching run ID. On VPN safety loss, take/drop the native session and remove telemetry before signalling the process. Normal exit/stop removes it when the run task ends.

`NetworkTraffic` fields are exactly: `project_id`, `service_id`, `run_id`, `profile_id`, `local_port`, `remote_host`, `remote_port`, `tx_bytes`, `rx_bytes`, `active_connections`.

- [ ] **Step 4: Register the desktop command**

Add `network_traffic` to `desktop.rs` and the Tauri invoke handler. Do not add an MCP tool.

- [ ] **Step 5: Run GREEN and Rust regression**

Run focused tests, then `rtk run '"${CARGO_HOME:-$HOME/.cargo}/bin/cargo" test --all-targets'` from `src-tauri` outside socket-restricted sandbox when required.

- [ ] **Step 6: Commit**

Commit `feat(network): publish current relay traffic` with only Task 2 files.

---

### Task 3: Non-overlapping polling and pulse state

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/api.ts`
- Create: `src/lib/useNetworkTraffic.ts`
- Create: `src/test/NetworkTrafficState.test.tsx`

**Interfaces:**
- Consumes: Tauri `network_traffic` snapshots from Task 2.
- Produces: `useNetworkTraffic(): { samples, activity, loading, error }`, where activity per route has independent `tx` and `rx` booleans.

- [ ] **Step 1: Write failing state tests**

With fake timers and deferred API promises, assert immediate first request, one-second self-scheduling after completion, no overlap, cleanup ignoring late responses, initial baseline without pulse, positive TX/RX deltas, independent 1.5-second expiry, counter reset/run change/missing record/error without phantom activity, and recovery.

- [ ] **Step 2: Run RED**

Run `rtk pnpm exec vitest run src/test/NetworkTrafficState.test.tsx --maxWorkers=1`.

- [ ] **Step 3: Add types and invoke**

Add the exact camel-case `NetworkTraffic` type and `api.networkTraffic()`.

- [ ] **Step 4: Implement the hook**

Use stable route key `projectId/serviceId/runId/localPort`. Poll only while mounted. A positive monotonic delta restarts only that direction's expiry. Clear baseline/activity on error, identity change, missing record, or decrease. Do not infer from `activeConnections`.

- [ ] **Step 5: Run GREEN and frontend regression**

Run the focused test, `rtk pnpm typecheck`, and the full frontend suite serially.

- [ ] **Step 6: Commit**

Commit `feat(network): derive live traffic pulses` with only Task 3 files.

---

### Task 4: Live Network overview page

**Files:**
- Create: `src/components/LiveNetwork.tsx`
- Create: `src/styles/live-network.css`
- Modify: `src/App.tsx`
- Modify: `src/components/Icon.tsx`
- Modify: `src/main.tsx`
- Test: `src/test/App.test.tsx`
- Create: `src/test/LiveNetwork.test.tsx`

**Interfaces:**
- Consumes: current `Workspace`, route configuration, VPN profile views, and Task 3 hook.
- Produces: sidebar view `network` and the accessible overview/detail UI.

- [ ] **Step 1: Write failing navigation and page tests**

Assert sidebar navigation/breadcrumb; configured routes remain neutral with no delta; TX/RX/both text and decorative pulse direction; idle open tunnel has no pulse; stopped route stays neutral; unsupported direct and Docker services say `Not observable`; keyboard route selection updates details; duplicate destinations remain distinct; route removal repairs selection; error says `Unavailable` without removing config.

- [ ] **Step 2: Run RED**

Run `rtk pnpm exec vitest run src/test/App.test.tsx src/test/LiveNetwork.test.tsx --maxWorkers=1`.

- [ ] **Step 3: Add navigation and semantic structure**

Add `network` to the App view union, sidebar utility button, breadcrumb, and conditional page render. Add one existing-style SVG `network` icon.

- [ ] **Step 4: Build the flow page**

Use native buttons and CSS grid/SVG only. Base lines are neutral. TX uses one blue outward dot; RX uses one violet return dot. SVG is `aria-hidden`; textual states and counters carry meaning. Resolve profile name from `workspace.vpns`; never render secrets. Services without native `vpnRoutes` go into `Not observable` without invented destinations.

- [ ] **Step 5: Add responsive/accessibility CSS**

Reuse restored glass tokens. Internal graph scrolls horizontally below its stable minimum width while the page does not. Reduced motion hides dots, forced colors preserves boundaries, focus remains visible, and selected/detail state is not color-only.

- [ ] **Step 6: Run GREEN**

Run focused tests, full frontend suite serially, `rtk pnpm typecheck`, `rtk pnpm lint`, and `rtk pnpm build`.

- [ ] **Step 7: Commit**

Commit `feat(network): add live flow overview` with only Task 4 files.

---

### Task 5: Transport proof and isolated acceptance

**Files:**
- Modify: `src-tauri/tests/vpn_docker.rs` only when needed to assert live telemetry in the existing synthetic native-route proof
- Create: `.agents/evidence/TASK-0026/live-network-flow.md`

**Interfaces:**
- Consumes: completed backend/frontend feature.
- Produces: bounded proof with no production mutation.

- [ ] **Step 1: Run all automated checks**

Run the frontend suite serially, typecheck, lint, frontend build, and Rust all-targets suite. Record actual counts.

- [ ] **Step 2: Prove actual transport telemetry**

Run or minimally extend the existing ignored synthetic test `native_vpn_routes_keep_commands_on_host_and_share_gateways` so known payload bytes are sampled while the route is alive, idle samples create no delta, and fixture cleanup still prints `fixtureCleanupComplete`.

- [ ] **Step 3: Run isolated Test app**

Use a fresh `SERVICE_MANAGER_DATA_DIR`, product `Service Manager Test`, Vite 1420, and MCP 47832. Confirm zero production projects before interaction and keep production 47831/PID unchanged.

- [ ] **Step 4: Verify page behavior**

Check light/dark/system, 30/55/100% opacity, 900×600 and larger windows, keyboard selection/focus, reduced motion, overflow, neutral idle lines, textual unsupported states, and no secret/private data.

- [ ] **Step 5: Clean up and write evidence**

Stop synthetic services/Test app, remove disposable data, confirm 1420/47832 closed and production listener unchanged. Write exact commands/results, test identity, observed states, limits, and remaining risks without raw private logs or tokens.

- [ ] **Step 6: Commit evidence**

Commit `test(network): verify live flow telemetry` with only the evidence/test files. Do not run `tauri build`, create a DMG, install, push, publish, or deploy.

