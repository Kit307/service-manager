# TASK-0011 - Drag service membership and correct UI icons

## Objective
Move services directly into groups or Ungrouped by dragging in the sidebar, preserving code, command definitions, roots and running processes. Correct misleading icons.

## Scope
Use native HTML drag/drop with a typed internal service reference and visible drop targets. Validate source and target natively and persist a single membership move atomically, without replacing other members. Settings remains the keyboard-accessible membership alternative. Handle invalid/foreign payloads, no-op moves, busy state, failed persistence and running-service moves. Use plus for Add service, stack for Compose, gear for Settings, grid for All services, group/list for Group views and a drag grip. App launcher artwork is unchanged.

## Verification
Regression tests for drag payload, group/ungrouped drop, invalid input, failed persistence and icon semantics/type safety. Rust tests for persistence/reload, duplicate IDs across roots, no-op moves and preserved state/config. Build v0.5.0 DMG and inspect it read-only. Do not modify the installed app.

## Completion
Implemented typed sidebar drag/drop, atomic native membership moves and semantic typed icons. React tests and real browser pointer checks passed; Rust tests preserve config bytes/running state and verify reload/failure rollback. v0.5.0 release and mounted DMG verification passed; temporary build bundle removed from LaunchServices and moved to Trash. Installed v0.4.0 and real services were preserved. See evidence for native-pointer and Docker/model-client limits.
