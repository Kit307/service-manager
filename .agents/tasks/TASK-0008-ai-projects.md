# TASK-0008 - AI project creation and opening

## Objective
Allow an MCP client to create a new project folder or register an existing code folder without first opening each project manually.

## Scope
Explicit per-client parent-folder grants in AI Connections; root discovery, project creation and project opening MCP tools; automatic access to projects registered by that client within its grants; tests, documentation/skill updates and macOS DMG.

## Non-Goals
Unrestricted filesystem access, deleting files, framework scaffolding, git initialization, automatic service execution, changing existing client grants, or automatic client installation.

## Dependencies
Existing token profiles, ProjectStore/Manager persistence, native folder picker and Streamable HTTP tools.

## Acceptance Criteria
- A client can be created with approved parent folders and no pre-opened projects.
- `list_project_roots` exposes only its granted roots.
- `create_project` creates one new child folder without overwriting an existing path; `open_project` opens an existing folder inside an allowed root.
- Reject absolute relative paths, traversal, out-of-root symlinks, replaced root paths, files and revoked/ungranted access before mutating a project.
- Projects appear in the desktop workspace and are granted only to the requesting client. Control/edit retain their separate existing permissions; opening/creation never executes commands.
- Legacy profiles deserialize with no root grants and keep their original access restrictions.
- Failed permission/persistence operations do not silently grant access or delete user files.

## Implementation Plan
1. Add failing protocol and client-profile UI tests.
2. Extend profile grants and native creation inputs; validate chosen directories.
3. Add scoped registration/create operations at AiBridge and expose MCP definitions.
4. Extend AI Connections to select roots and bootstrap with no projects.
5. Update contract/skill and README; run focused and adjacent checks.
6. Build one final v0.3.0 app/DMG, verify it, and remove temporary app bundles to avoid duplicate icons.

## Verification
Real MCP SDK integration tests with temporary roots; UI interaction tests; frontend lint/build, Rust formatting/Clippy, profile compatibility tests; mounted DMG inspection.

## Stop Conditions
Need for unrestricted access, external deployment, credentials, or destructive migration.

## Risks
Root grants authorize discovery/registration within selected directories; project access is paired with the explicitly selected control/edit grants. Filesystem paths must remain confined. Do not infer trust from opening a folder.

## Evidence Required
`.agents/evidence/TASK-0008/` with test results and packaging checks.

## Completion Summary
Implemented scoped AI project creation/opening, root-only client bootstrap, native/React grant inputs, MCP discovery, legacy-profile compatibility and documentation/skill updates. Tested 22 unique Rust cases and 12 React cases; one opt-in Docker case remained ignored. Lint, formatting, Clippy, production build, skill validation and mounted DMG checks passed. Packaged v0.3.0 and updated the previously installed Codex/Claude skill copies with backups. The installed application and personal AI connection settings were left unchanged. Evidence: `.agents/evidence/TASK-0008/ai-projects.md`.
