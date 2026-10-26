# MILESTONE-0003 - Shared Docker VPN workspaces

## Objective
Run selected command services through shared OpenVPN tunnels in Docker, with multiple VPNs active simultaneously and no Mac VPN/routing changes.

## Scope
Private shared VPN profiles, imported inline .ovpn plus private username/password files, Gluetun gateway lifecycle/ownership, per-command VPN container image, shared existing Compose logs/history/Stop, UI and MCP profile discovery, tests and docs.

## Non-Goals
Host VPN automation, arbitrary existing Compose rewrites, automatic conversion of host-specific commands, OTP/SSO, image builds, remote Docker, changes to running user services, silent deployment.

## Expected Outcomes
A/B VPNs have separate namespaces. Services sharing a profile share one gateway. Start waits for tunnel health; no host execution fallback. Stopping one service preserves other users. Disconnect refuses active users. Worker DNS uses the tunnel namespace; published ports bind loopback only.

## Dependencies
Local Docker daemon; local qmcgaw/gluetun:v3.41.0 and user-selected Linux runtime images; user imports inline-certificate .ovpn and credentials privately. Actual organization VPN testing requires user selection/authorization; synthetic fixtures must not use organization networks or secrets.

## Tasks
- TASK-0018 - Shared Docker VPN service runtime

## Completion Criteria
End-to-end UI/config/runtime path and regressions pass; real Docker tests establish sharing/isolation/no-route-fallback where available; any real-VPN or install limitation is explicit.

## Completion
TASK-0018 implemented and verified with synthetic Docker VPNs. App/DMG 0.8.0 built; installed app activation is intentionally separate. See task record and evidence for proof and network-specific limits.

## Authentication follow-up
TASK-0019 adds certificate-only and encrypted-key authentication compatibility in 0.8.1, with synthetic regression proof before real profile setup.

## Interactive challenge follow-up
TASK-0020 implements separately approved dynamic CRV1 MFA/PIN support in 0.8.2 for independent simultaneous VPN profiles.
