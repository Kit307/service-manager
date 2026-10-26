# TASK-0018 - Shared Docker VPN service runtime

## Objective
Add shared Docker OpenVPN profiles and opt-in command containers without changing native/Compose service behavior.

## Scope
vpn profile manager, Docker helper reuse, optional service vpn {profileId,image}, private imports/secrets, generated worker Compose, model/API/UI/MCP read-only profile discovery, release docs and tests.

## Non-Goals
Host network changes, imported real credentials in fixtures, automatic running-service migration, OTP/SSO, arbitrary Compose networking edits.

## Dependencies
MILESTONE-0003; existing manager Compose lifecycle and trust/revision model.

## Acceptance Criteria
- Profiles are imported locally; public metadata never returns credentials/config contents.
- One healthy owned gateway per profile; parallel service starts cannot duplicate it. Separate profiles can stay connected concurrently.
- Service start acquires a lease before generating a worker with network_mode container:<immutable gateway ID>, all capabilities dropped and tunnel DNS resolver mounted read-only.
- Native fallback is impossible for a VPN-selected service. Tunnel failure stops its owned workers and keeps unrelated profiles intact.
- Closing/stopping one worker releases only its lease. VPN disconnect/delete refuses active Docker namespace users, including after app restart.
- VPN host ports map to container ports and bind only 127.0.0.1. No host tunnel or routing commands.
- Existing absent vpn fields serialize identically; Compose+vpn and unsafe inputs are rejected.
- Current user services and VPN remain untouched during implementation.

## Verification
Focused model/gateway/config/ownership/cancellation tests, UI tests, nearest full frontend/native gates, opt-in synthetic Docker tests. Credentials never enter source, CLI arguments, logs or public artifacts.

## Stop Conditions
Do not activate organization VPNs or stop current services; report missing real VPN test prerequisites. Do not ship a claim of leak prevention without exercising packet/DNS loss behavior.

## Risks
Privileged NET_ADMIN is limited to the owned Gluetun container inside Docker VM. Profiles require inline certificate material and remembered username/password. Linux command/image compatibility is explicit. Docker-admin access can inspect private mounts; VPN is not a sandbox for hostile code.

## Evidence Required
Test results and package metadata; synthetic only.

## Completion Summary
Implemented optional shared Docker VPN command execution, private OpenVPN profiles, corporate DNS/ICMP settings, hostname-to-IPv4 runtime normalization, pinned local daemon ownership, verified worker cleanup and UI/API/MCP discovery. No new application dependencies.

Verification: 96 frontend tests, 68 native tests, explicit Compose and production-Manager VPN Docker fixtures, desktop check and macOS arm64 0.8.0 release build passed. Two simultaneous synthetic VPNs and three workers verified sharing, run reuse, post-crash disconnect guard and isolated loss cleanup. See evidence/TASK-0018.

DMG prepared outside the repository at ../outputs/Service-Manager-0.8.0-macOS-arm64.dmg. Installed app/services remain unchanged. Real organization authentication/DNS, blackholed-tunnel timing and interactive UI visual QA remain unverified; existing gateways require remove/reimport after remote DNS rotation. Source docs/skill updated; installed skill/app activation remains separate.
