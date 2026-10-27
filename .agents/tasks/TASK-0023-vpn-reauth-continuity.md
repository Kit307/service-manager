# TASK-0023 - VPN reauthentication continuity

## Objective
Keep an already running VPN worker alive through a transient OpenVPN reconnect or PIN challenge without allowing traffic to bypass the gateway firewall.

## Scope
Separate tunnel readiness required before service start from gateway ownership safety required after start; add focused unit and synthetic Docker regression coverage and document the runtime behavior.

## Non-Goals
Automatic PIN entry, saved PINs, browser SSO, host-network fallback, disabling health checks, or changing the provider's reconnect policy.

## Dependencies
Existing per-profile authenticated management bridge, owned Docker gateway, worker network-namespace sharing, and firewall kill switch.

## Acceptance Criteria
- A new PIN challenge or transient unhealthy state does not stop workers while the exact owned gateway container remains running.
- Workers cannot fall back to the Mac network while the tunnel is unavailable.
- A stopped gateway or ownership, fingerprint, or identity change still stops its workers.
- New service starts still require a ready VPN.

## Verification
Focused Rust unit test, complete Rust library tests, synthetic Docker gateway pause/stop proof, formatting, Clippy, and frontend regression checks.

## Stop Conditions
Stop before persisting credentials/PINs, weakening ownership checks, adding host fallback, or changing installed/running services without separate approval.

## Risks
Requests from a preserved worker fail while reauthentication is pending. A user must still enter each new PIN in the desktop app.

## Evidence Required
RED/GREEN unit output, Docker proof showing worker preservation while unhealthy and cleanup after gateway stop, and final test/build results.

## Completion Summary
Added a worker-safety check that keeps an active worker attached while its exact owned gateway remains running, including transient unhealthy and PIN-required reconnect states. New starts still require a ready tunnel; stopped, replaced, fingerprint-changed, or ownership-lost gateways still stop workers.

Verification completed:
- RED/GREEN focused unit regression.
- Rust library suite: 33 passed, 0 failed, 1 ignored.
- Synthetic Docker pause/stop proof: preserved worker while unhealthy; stopped it after gateway stop; second VPN unaffected.
- Synthetic CRV1 MFA proof passed.
- React tests: 106 passed.
- TypeScript, ESLint, frontend production build, and native macOS app bundle passed.
- Clippy passed with three unrelated existing findings allowed. Strict Clippy and `cargo fmt --check` still report pre-existing issues outside this task.

No app installation or Service Manager restart was performed, so current local services remain running. The focused fix is intentionally uncommitted because its files also contain pre-existing 0.8.3 work that cannot be separated safely without mixing logical tasks.
