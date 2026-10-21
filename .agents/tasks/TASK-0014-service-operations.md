# TASK-0014 - Shared service operations

## Objective
Implement the MILESTONE-0002 backend behaviors in the existing manager.

## Acceptance Criteria
- Existing definitions still parse and retain their old serialization/fingerprint.
- Declared port conflicts are detected before spawn; inspection failures are not reported as free ports.
- Confirmed Kill validates port, PID, owner and process identity again; owned services are stopped through the manager.
- Dependency plans reject cycles before side effects, wait for Ready, support cancellation, and stop in reverse order.
- History retains bounded private per-run evidence and survives manager recreation.
- Loopback URLs are validated and checked before native opening.

## Verification
Rust fixtures for held ports, changed identities, process ownership, dependency readiness/cycles/cancellation, history bounds and stale run IDs. Preserve existing lifecycle, root, config and MCP tests.

## Completion Summary
Implemented in the shared Manager, model/config boundaries, port inspection and bounded private history. Verified 51 full-suite Rust tests with one opt-in Docker case ignored, including deterministic preflight cancellation. Final frontend integration has 81 passing tests; real SDK/proxy covers new evidence tools and revocation. Independent review findings for confirmed-run Stop and fallible URL lookup were resolved and rechecked. See [verification](../evidence/TASK-0014/verification.md). Native platform commit: f911c92.
