# TASK-0001 - Desktop workspace and first working service flow

## Objective
Build the agreed UI and connect it to the local Rust manager.
## Scope
Native folder selection, config editing, command lifecycle, logs, and the shared AI access boundary.
## Non-Goals
Remote/shared infrastructure and public release.
## Dependencies
Existing approved design/plan and macOS tools.
## Acceptance Criteria
No fake status; native controls operate on owned processes; opening a folder does not execute code; log rendering remains bounded.
## Verification
React behavioral tests, Rust filesystem/lifecycle tests, typecheck, lint, native build and manual smoke checks.
## Stop Conditions
Credentials, shared infrastructure, unsafe external configuration, or public publishing required.
## Risks
Shell commands have user permissions; app crash cannot guarantee daemonized child cleanup.
## Evidence Required
Sanitized test/build results and native UI inspection.
## Completion Summary
Desktop UI, native folder boundary, command-service persistence/control, bounded live log reads, readiness, and authenticated MCP/proxy are implemented. Five React and thirteen Rust tests, typecheck, lint, Clippy, Rust formatting, and the 13.32 MiB arm64 release build passed. Independent review findings were fixed and rechecked. Evidence is in .agents/evidence/TASK-0001/verification.md. Complete target-system acceptance remains open as listed in docs/verification.md; this UI delivery does not mark the full implementation plan complete. Source and lockfiles form one focused commit, reported in the delivery response. Generated builds, private state, and user-facing outputs are intentionally outside source control.
