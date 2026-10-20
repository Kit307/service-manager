# TASK-0007 - Remove a project from the workspace

## Objective
Allow the desktop user to remove a project from Service Manager while preserving its code folder and saved service configuration.

## Scope
Sidebar removal control, confirmation, native command, persisted workspace removal, focused regression tests, and a new macOS DMG.

## Non-Goals
Deleting project files, stopping services automatically, granting AI permission to remove projects, or replacing the installed app.

## Dependencies
Existing ProjectStore persistence, Manager lifecycle locking, Tauri commands, and React workspace refresh.

## Acceptance Criteria
- Each project has an accessible Remove project control and confirmation describing preserved files.
- Starting, running, ready, or stopping services block removal at the shared store lock.
- Removed projects stay absent after restarting the manager.
- Reopening preserves saved services but uses a new project identity and requires fresh trust/AI grants.
- Persistence errors retain the project and its private state.
- Removing the selected project selects another project or returns to the empty workspace.

## Verification
Focused Rust persistence/lifecycle tests; React interaction tests; lint, typecheck/build, Clippy, formatting; packaged DMG inspection.

## Stop Conditions
Unexpected user-data deletion, failed lifecycle cleanup, or required deployment/credential changes.

## Risks
Concurrent AI starts must not admit work after removal; reopening must not restore old project grants.

## Evidence Required
Test results and bundle/DMG verification in `.agents/evidence/TASK-0007/`.

## Completion Summary
Implemented desktop project removal with confirmation, active-service protection, durable persistence, and preservation of code/config files. Reopening uses fresh trust/AI grants; Compose resource behavior is disclosed before removal. React tests (11), focused Rust tests (9), lint, Clippy, formatting, and final production build passed. Packaged and mounted the v0.2.4 DMG successfully; removed temporary app bundles to avoid duplicate icons. Evidence: `.agents/evidence/TASK-0007/remove-project.md`. Installed app was left unchanged.
