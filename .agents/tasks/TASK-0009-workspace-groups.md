# TASK-0009 - Groups, collapsible navigation and group controls

## Objective
Replace the visible Project layer with Group → Service while preserving every existing service, command, working root and owned process.

## Approved Design
- A group contains services directly. Internal project identities remain for working roots and MCP compatibility.
- Create, rename and remove groups in the app. Membership selection moves a service from any previous group. Removing a group leaves services ungrouped and does not stop services or touch code.
- Existing service removal retains confirmation, active-service protection and preserved code files.
- Separate disclosure buttons collapse groups. Remember disclosure state locally across app restarts.
- Clicking a group name opens its overview; clicking a service opens its output.
- Run all starts stopped/failed configured services; Stop all signals all active owned services; Restart all restarts configured services. Use the existing native service controls rather than adding a second lifecycle implementation.
- Review all untrusted target definitions before submitting any starts/restarts. Cancel means no service action. Report action failures per service and display live status; accepted requests do not imply readiness.

## Scope
Private persisted group metadata, native CRUD commands, sidebar, group editor, group overview, UI batch orchestration using existing controls, tests/docs and v0.4.0 DMG.

## Non-Goals
Filesystem folders, deleting code, dependency orchestration, scheduling, new native/MCP group lifecycle APIs, new MCP group grants, or the separately discussed persistent MCP/profile-editing enhancements.

## Acceptance Criteria
- Existing workspace state opens without migration loss.
- Group CRUD and membership persist atomically; invalid members and failed writes do not change groups.
- Removing a service removes its membership; removing a group preserves its services.
- Disclosure controls work with mouse/keyboard and preserve their state.
- Group overview exposes individual status/log access and all three bulk controls.
- Bulk control skips already active services on Run all, reports partial failures, preserves services outside the group and respects existing trust/revision/lifecycle boundaries.

## Verification
Regression-first React tests for disclosure and bulk actions; Rust group persistence tests; existing lifecycle/MCP tests, lint, formatting, Clippy, production build and mounted DMG inspection.

## Stop Conditions
Need to delete user files, broaden existing AI permissions, migrate live resources or deploy externally.

## Risks
Groups are organizational only and must never imply AI access grants. Bulk requests can partly fail; show actual states instead of claiming all services are ready.

## Evidence Required
`.agents/evidence/TASK-0009/` with verification results.

## Completion Summary
Implemented Group → Service UI, persisted virtual groups/membership and disclosure, group overview and scoped bulk controls using the existing manager. Existing definitions, roots, trust and owned processes are retained. React 18/18 and Rust 26/26 tests passed (one opt-in Docker case ignored), lint/build/format/Clippy passed, v0.4.0 release and mounted DMG verified. See evidence for browser/native limits and artifact receipt. Installed app was preserved; generated build bundle moved to Trash to avoid duplicate icons.
