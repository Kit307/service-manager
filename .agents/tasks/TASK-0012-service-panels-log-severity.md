# TASK-0012 - Correct log severity and retain the service shell in Settings

## Objective
Display stdout/stderr as transport streams without claiming informational stderr is an error. Settings must occupy the same service content pane as Output and preserve the service heading, status and controls.

## Scope
Frontend presentation and the native window integration. Label streams explicitly. Color only recognized severity prefixes or a JSON string level; unknown severity is neutral. Do not infer severity from arbitrary message text, HTTP status or stream. Retain raw log content/stream, search/filter/copy behavior and the MCP/proxy contract. Unknown logger formats remain neutral.

Keep the log view mounted across tab switches so filters/follow/history survive. Capture an editor session's definition/revision on first edit, retain it across tab switches, and discard it only on explicit Cancel, successful Save or target navigation. Background refresh must not advance a draft's expected revision. Creation stays a standalone form.

Native follow-up: retain disabled Dock minimization as previously requested; allow edge resizing down to 900 x 600, never smaller. Remove the separate macOS title strip with Overlay/hidden title, retain native traffic lights and reserve a draggable top area. Replace HTML service dragging with captured pointer movement so membership moves work independently of native file drag interception. Retain the native file handler default. No filesystem drag/import feature is added.

## Verification
Regression-first tests for informational stderr, explicit error/warning stdout/stderr, unknown/malformed messages, stream filtering and inert text. Tests for the persistent heading/controls, Output/Settings switching, retained logs/search and stale draft revisions. Frontend tests/lint/build, synthetic browser checks, release DMG verification.

## Safety and non-goals
No real service commands, log access, token changes, process restarts or source-project edits. No backend log schema, logger installation, configurable parsers, full-app redesign.

## Completion
37 frontend tests, ESLint, TypeScript and release build passed. Isolated native WebKit verified group removal/reassignment and direct cross-group moves, overlay titlebar, and edge resizing clamped to 900 × 600. See evidence for limits and packaging receipt.
