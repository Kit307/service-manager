# TASK-0024 - Native service VPN destination relays

## Objective
Keep command services native on macOS while routing explicit TCP destinations through shared Docker OpenVPN gateways.

## Scope
Service-level destination routes, loopback TCP relays through the selected Gluetun HTTP proxy, shared profile leases, fail-closed lifecycle handling, route UI, synthetic proof, and an isolated Service Manager Test app.

## Non-Goals
Moving Go/Node into Docker, UDP, full subnet interception, DNS rewriting, VPN-to-VPN bridging, macOS Network Extension, editing application source, installing the final app, or restarting current services.

## Dependencies
Existing VPN profile import/authentication, owned Gluetun gateway lifecycle, native command manager, Docker Desktop, and local synthetic VPN fixtures.

## Acceptance Criteria
- Original native command/cwd/env/log behavior remains intact.
- One service can use routes from two VPN profiles; several services can share one profile.
- Only declared `127.0.0.1:<localPort>` endpoints use VPN.
- Missing/unhealthy/replaced gateways prevent start or stop the active routed service without direct-network fallback.
- Relay listeners disappear on Stop and never bind publicly.
- Test app uses separate state, identifier, single-instance scope, and MCP port 47832.
- Automated tests and isolated runtime proof pass before production build.

## Verification
Focused RED/GREEN Rust and React tests, full Rust/React/typecheck/lint/build, synthetic two-VPN Docker proof, test-app process/listener/state inspection, then production bundle build.

## Stop Conditions
Stop before touching installed app data, current services, real VPN credentials, production-like networks, public listeners, or application source/environment values.

## Risks
Applications must point raw TCP endpoints at the configured loopback relay. Existing long-lived connections close when a gateway disappears. Corporate proxy policy or DNS behavior still requires a real-profile check after synthetic proof.

## Evidence Required
RED/GREEN output, automated suite results, test-app identifier/data/MCP evidence, native PID/log evidence, two isolated VPN responses, shared gateway count, and relay cleanup after Stop.

## Completion Summary
Pending.
