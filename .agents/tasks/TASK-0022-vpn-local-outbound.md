# TASK-0022 - VPN local outbound allowlist

## Objective
Allow a VPN-routed service to reach an explicitly approved private IPv4 subnet on the Mac without weakening the VPN firewall globally.

## Scope
Per-profile private IPv4 CIDR validation and persistence, Gluetun `FIREWALL_OUTBOUND_SUBNETS`, the existing stopped-profile network editor, focused tests, app rebuild, and AWS DEV runtime verification against Merchant Profile.

## Non-Goals
Public/LAN exposure, `0.0.0.0/0`, automatic route discovery, IPv6, reading env files, or changing unrelated services.

## Dependencies
Existing AWS DEV VPN profile, Docker host route `192.168.5.0/24`, Merchant Profile on `127.0.0.1:8081`, and the existing Service Manager VPN/runtime implementation.

## Acceptance Criteria
- Only canonical RFC1918 IPv4 CIDRs are accepted; duplicates, host-bit CIDRs, loopback and `0.0.0.0/0` are rejected.
- The gateway receives only the configured CIDRs through `FIREWALL_OUTBOUND_SUBNETS`.
- Existing profiles load with an empty allowlist.
- Network edits retain profile identity and private credentials and still require stopped services plus a disconnected gateway.
- Merchant BFF reaches Merchant Profile through `host.lima.internal:8081`; `/v1/me/organizations` no longer fails because of local upstream reachability.

## Verification
Focused Rust and React tests, build/typecheck, installed app version, gateway environment inspection without secrets, service status/logs, and loopback endpoint checks.

## Stop Conditions
Stop before public exposure, a broad outbound CIDR, credential access, foreign container removal, or any shared/deployed environment change.

## Risks
Restarting the AWS DEV gateway interrupts its attached local services. A dynamic VPN challenge may require user input.

## Evidence Required
Test output, runtime gateway CIDR, service run IDs/status, BFF-to-Merchant-Profile ping, and the organization route result.

## Completion Summary
Pending.
