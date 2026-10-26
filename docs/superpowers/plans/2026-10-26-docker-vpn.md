# Shared Docker VPN Implementation Plan

> **For agentic workers:** Use subagent-driven tasks with scoped ownership; integrate and review before release.

**Goal:** Share independent OpenVPN Docker gateways across selected service containers while keeping the Mac off VPN.

**Architecture:** VpnManager owns private profiles, gateway ownership/pinned local Docker sessions and leases. Existing Compose lifecycle owns each generated worker; it shares the gateway network namespace and mounts a fixed tunnel resolver. Native services and user Compose files retain their existing execution path.

**Tech Stack:** Rust/Tokio/serde, existing Tauri/React, Docker CLI, Gluetun v3.41.0. No new application package dependencies.

## Global Constraints
- No host VPN/routing changes, real secrets in tests, automatic service migration, OTP/SSO or remote Docker.
- Local .ovpn imports and credential files are private; no credential/config payload in public metadata/logs/MCP.
- Runtime images must exist locally; explicit UI download handles preparation separately.
- A service uses optional vpn: {profileId, image}; only command services support it. A host-specific command must be edited to a Linux-compatible one before running in its chosen image.

### 1. Shared Docker and VPN profile manager
- [x] Extract pinned local DockerSession/capture helper; preserve Compose errors and behavior.
- [x] Implement private imports, validation, owned gateways/bridges, healthy connect, shared leases and disconnect refusal.
- [x] Test validation, private data, ownership, concurrent leases, restart discovery and failure/cancellation.

DTO contract: VpnImport {name,configPath,username,password,publishedPorts:[{host,container}],healthTarget,dnsAddress?,healthIcmpTarget?}. VpnProfileView {id,name,publishedPorts,healthTarget,dnsAddress,healthIcmpTarget,state,error?,users:[{projectId,serviceId}]}. VpnManager::new(PathBuf); async list/import/connect/disconnect/remove/acquire/shutdown. VpnLease exposes session, gateway_id and published_ports; async healthy() -> Result<bool>; Drop releases only this run's lease.

### 2. Service execution and schema
- [x] Add optional VpnService {profileId,image}; validate command-only and port/image constraints; preserve absent-field fingerprints.
- [x] Acquire gateway before generating a one-service private Compose snapshot; use existing lifecycle/logs/history and run identities.
- [x] Monitor gateway health, stop only affected owned workers on loss, never run a host fallback, and disconnect owned gateways only after worker shutdown.
- [x] Exercise lifecycle, stop/cancel, shared users and dependency behavior.

### 3. UI and APIs
- [x] Add VPN workspace navigation and private .ovpn import form; connect/disconnect/remove, explicit image downloads and profile/user health states.
- [x] Add command-service VPN/image controls with Linux-command and port guidance; no credentials in project.json.
- [x] Bridge desktop commands and add list_vpn_profiles read-only MCP discovery; same service upsert/start APIs.
- [x] Test UI state/error preservation, native/Compose compatibility, save validation and redaction.

### 4. Verification, docs and delivery
- [x] Real Docker synthetic sharing/isolation/fail-closed checks; no organization VPN without explicit selected profile.
- [x] Full frontend/native gates, independent code review, skills/docs updates.
- [x] Build a fresh-version app/DMG; keep current installed manager/services running until activation is separately agreed.

## Verification boundaries

Synthetic Manager tests cover two VPNs, three workers, namespace sharing and gateway-stop cleanup. The tunnel-loss case stops a gateway; blackholed UDP timing, real organization authentication and corporate DNS remain unverified. Gluetun permits its owned Docker bridge subnet. Post-crash user lists are lease-based; disconnect/remove separately scan real namespace users. Hostname remotes resolve to IPv4 before new gateway creation.
