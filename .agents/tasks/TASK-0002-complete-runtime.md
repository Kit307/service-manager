# TASK-0002 - Complete the local runtime

## Objective
Finish typed Compose execution and verify the shared desktop/AI workflow.
## Scope
Owned local Compose lifecycle, bounded logs, health recovery, UI updates, local client setup, and a new release artifact.
## Non-Goals
Remote Docker, public release, signing, automatic edits to personal AI settings.
## Dependencies
TASK-0001 and installed local Docker/AI clients.
## Acceptance Criteria
Compose only controls verified owned containers; stop uses the immutable launch configuration and preserves volumes. AI and desktop observe the same runs. Logs remain bounded. Release checks are reported with actual evidence.
## Verification
Focused Rust integration tests, React tests, typecheck/lint, native build, isolated Docker and client smoke checks.
## Stop Conditions
Missing credentials, changes to shared resources, deployment, or destructive cleanup outside task fixtures.
## Risks
Commands and reviewed Compose files execute with user permissions. Unsigned local macOS build.
## Evidence Required
Sanitized test results and release checks.
## Completion Summary
Implemented Compose, event-driven desktop updates, health deadline/recovery, UTF-8 flood handling, private AI setup snippets and native controlled Quit. Rust/React, live Docker, actual Codex MCP/proxy and build checks are recorded in `.agents/evidence/TASK-0002/verification.md`. Claude real-client use is deferred by explicit user choice pending login. Idle CPU/RSS was measured for 62 seconds: 0.322% average CPU and 120–158 MiB summed RSS. Startup and full-app flood benchmarks remain unverified; the peak RSS exceeds the 150 MiB goal. Source and documentation will be committed as one runtime completion increment; generated artifacts and private test state remain outside Git.
