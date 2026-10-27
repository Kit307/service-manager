# TASK-0021 - Edit VPN published ports without reimport

## Objective
Let an existing VPN profile publish the localhost ports required by its services without importing the profile or re-entering saved credentials.

## Scope
Ports-only profile updates, safe stopped-gateway replacement, desktop UI/API, removal of the incorrect automatic Node runtime choice, focused verification, and local runtime setup for Asgard and Merchant BFF go.

## Non-Goals
Automatic conversion of arbitrary macOS commands or binaries to Linux, host VPN/routing, public port binding, MCP mutation, or weakening certificate checks.

## Dependencies
Docker Desktop/Colima, the existing two private VPN profiles, and user-entered MFA/PIN when AWS reconnects.

## Acceptance Criteria
- Existing profile ID, private config, credentials, passphrases, network ownership and health settings remain unchanged.
- Published ports can change only with no active service lease, pending authentication, running gateway, or running namespace user.
- Only a verified owned stopped gateway is removed; foreign resources are preserved.
- UI validates mappings and does not guess Node for a Go or unknown command.
- Krit can publish `127.0.0.1:3000 -> 3000`; AWS can publish `127.0.0.1:8080 -> 8080`.

## Verification
Focused Rust and React tests, Docker runtime regression, build/typecheck, native UI smoke check, and localhost health checks after service restart.

## Stop Conditions
Do not interrupt an active shared VPN. Ask the user to enter dynamic PIN in the app; never collect it in chat.

## Risks
Docker cannot change a container's published ports in place, so a stopped owned gateway must be replaced. Reconnect can require MFA.

## Evidence Required
Test output, installed app version/hash if rebuilt, service run IDs/status, and actual localhost response for ports 3000 and 8080.

## Completion Summary
Pending.
