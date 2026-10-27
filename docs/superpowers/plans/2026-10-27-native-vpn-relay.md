# Native VPN Relay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep Go/Node commands native on macOS while routing explicit local TCP endpoints through one or more shared Docker OpenVPN gateways.

**Architecture:** A command service may declare destination routes (`profileId`, `remoteHost`, `remotePort`, `localPort`). Service Manager keeps the original shell command and env unchanged, starts a loopback TCP relay for each route, and sends each connection through the selected Gluetun gateway's loopback-only HTTP CONNECT proxy. A service run holds one shared gateway lease per profile; gateway ownership loss stops the service and relay so traffic fails closed.

**Tech Stack:** Rust/Tokio, Docker/Gluetun, Tauri 2, React/TypeScript, Vitest.

## Global Constraints

- Existing command, cwd, env file, Run/Stop/Restart, logs, health checks, and direct Mac networking stay unchanged.
- Docker contains only VPN gateways; application processes never move into Docker.
- Only configured TCP destinations use VPN. UDP, transparent per-process routing, and VPN-to-VPN bridging are excluded.
- Relay listeners bind only to `127.0.0.1`.
- Existing installed Service Manager and its running services must not be stopped or restarted.
- Test app uses a different Tauri identifier, app-data directory, single-instance scope, and MCP port.

---

### Task 1: Route contract and validation

**Files:**
- Modify: `src-tauri/src/model.rs`
- Modify: `src/lib/types.ts`
- Test: `src-tauri/tests/vpn_service.rs`

**Interfaces:**
- Produces: `VpnRoute { profile_id, remote_host, remote_port, local_port }`
- Produces: `ServiceDefinition.vpn_routes: Vec<VpnRoute>` serialized as `vpnRoutes`

- [ ] Add failing Rust tests proving command services accept multiple profiles and reject compose routes, invalid hosts, zero ports, and duplicate local ports.
- [ ] Run `rtk cargo test --test vpn_service` and confirm RED because `vpnRoutes` is unsupported.
- [ ] Add the minimal model and validation, preserving the no-field wire format when routes are empty.
- [ ] Update the TypeScript contract and rerun the focused test to GREEN.

### Task 2: Loopback HTTP CONNECT relay

**Files:**
- Create: `src-tauri/src/vpn_relay.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/vpn_relay.rs`

**Interfaces:**
- Produces: `VpnRelaySet::start(routes, proxy_ports)` which binds every loopback listener before returning.
- Produces: `VpnRelaySet::safe()` and Drop cleanup for listeners/tasks and gateway leases.

- [ ] Add a failing integration test using a real local fake CONNECT proxy and TCP echo target.
- [ ] Run the focused test and confirm RED because the relay module does not exist.
- [ ] Implement bounded CONNECT response parsing, 200-only acceptance, bidirectional copy, loopback binding, and cleanup.
- [ ] Add failure tests for non-200 proxy responses and occupied local ports; run focused tests GREEN.

### Task 3: Shared gateway proxy and native command lifecycle

**Files:**
- Modify: `src-tauri/src/vpn.rs`
- Modify: `src-tauri/src/manager.rs`
- Modify: `src-tauri/src/ports.rs`
- Modify: `src-tauri/tests/vpn_service.rs`
- Modify: `src-tauri/tests/vpn_docker.rs`

**Interfaces:**
- `VpnLease.proxy_port: u16` is a random loopback host port mapped to Gluetun HTTP proxy port 8888.
- Routed command preparation acquires one lease per distinct profile and starts native shell execution only after every relay is listening.

- [ ] Add failing tests showing route local ports participate in conflict detection and service save rejects missing profiles.
- [ ] Enable Gluetun HTTP proxy on loopback and discover its assigned host port after verified gateway creation.
- [ ] Route command services through the existing native spawn path; retain leases/relays for the run lifetime.
- [ ] Stop the native process if exact gateway ownership/identity is lost; never fall back to the Mac network.
- [ ] Extend synthetic Docker proof for two profiles, one shared profile, native process identity, cleanup, and gateway loss.

### Task 4: Service editor

**Files:**
- Modify: `src/components/ServiceEditor.tsx`
- Modify: `src/styles/vpn.css`
- Test: `src/test/ServiceEditor.test.tsx`
- Modify: `src/components/VpnSettings.tsx`
- Test: `src/test/VpnSettings.test.tsx`

**Interfaces:**
- Service Network editor owns destination routes.
- VPN settings retain profile import/connect/authentication and stop presenting service runtime images as required.

- [ ] Add failing UI tests for adding/removing routes, selecting different profiles, and saving exact numeric ports.
- [ ] Replace the single runtime-image VPN form with repeatable route rows and concise endpoint guidance.
- [ ] Remove obsolete image download/runtime copy from the service editor while preserving VPN profile lifecycle controls.
- [ ] Run focused Vitest tests GREEN, then typecheck and lint.

### Task 5: Isolated test app and verification

**Files:**
- Create: `src-tauri/tauri.test.conf.json`
- Modify: `src-tauri/src/desktop.rs`
- Modify: `package.json`
- Modify: `docs/docker-vpn.md`
- Add: `.agents/evidence/TASK-0024/` runtime notes only when proof runs.

**Interfaces:**
- Test product: `Service Manager Test`
- Test identifier: `dev.servicemanager.desktop.test`
- Test MCP port: `47832`

- [ ] Make MCP port configurable with a validated environment variable; production remains 47831.
- [ ] Run Rust tests, React tests, typecheck, lint, and frontend build.
- [ ] Launch only the isolated test app and verify its data directory and MCP listener do not overlap production.
- [ ] In the test app, verify native command PID/logs plus two synthetic VPN destinations and Stop cleanup.
- [ ] After the test app passes, build the production app bundle without installing or restarting it.
