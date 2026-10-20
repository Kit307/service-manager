# AI project creation/opening — 2026-10-20

## Behavior

- AI Connections permits a profile with allowed parent folders and zero pre-opened projects. The native picker selects roots; the backend canonicalizes and validates directories. Existing profiles default to no root grants and retain their original permissions.
- Added MCP `list_project_roots`, `create_project {root,name}`, and `open_project {root,relativePath}`. Creation makes one new empty child folder without overwriting anything. Opening reads existing configuration. Neither action initializes git, scaffolds framework files, trusts imported services, or starts commands.
- Registration automatically grants only the requesting client access using its existing control/edit flags. Client lookup, root authorization and grant persistence are serialized with revocation through the clients lock. ProjectStore confines resolved project paths to the selected root; absolute relative paths, parent traversal, outside symlinks, replaced root symlinks, existing create targets, and files are rejected.
- Grant persistence uses the existing secure-write path and publishes updated in-memory grants only after a successful write. On failure, created folders/local registration may remain; no user files are deleted and no client grant is added. The contract describes recovery through opening the existing intended folder after fixing storage.
- Updated MCP contract, setup guide, source skill and its Codex/Claude metadata. Backed up and updated the three previously installed skill files only after comparing both installed copies against the tracked baseline. No personal MCP configuration, tokens or grants were changed.

## Verification

- Red phase: new SDK test failed with `CONFIG_INVALID: projectId is required` when requesting the absent root-list tool. New UI test failed because Add allowed folder was absent.
- Green phase: real Streamable HTTP SDK test passed for root discovery, empty-workspace creation, existing-folder opening, repeated opening, no overwrite, scope rejection, per-client isolation, service editing without execution, separate control permission, durable project grants and token revocation.
- Additional tests passed for root-only profile creation, invalid directory grants, legacy profile defaults, replaced-root symlink rejection and grant-write failure preserving both files and the absence of client access.
- Full default Rust suite: 21 passed before the additional persistence-failure case; final focused AI-project suite: 4 passed. This covers 22 unique Rust tests. One opt-in Docker integration test remained ignored; no Docker resources were modified.
- React suite: 12 passed, including bootstrapping a client with no projects, adding/removing allowed folders, profile submission, and all project-removal regressions.
- Frontend lint/build, Rustfmt check, final Clippy `--all-targets -- -D warnings`, version/window configuration assertions, and `git diff --check` passed.
- Source and both installed skill copies passed quick_validate using an isolated uv/PyYAML tool environment. No application dependency was added. Versioned skill ZIP integrity passed.
- Final Tauri production build passed (v0.3.0, 15.52 MiB app bundle). Created `Service-Manager-0.3.0-macOS-arm64.dmg` (8,063,856 bytes). DMG checksum and read-only mounted checks passed for version, exact ICNS, executable permission and Applications shortcut. Ejected the image and moved only two temporary app bundles to recoverable Trash, retaining the installed app.

## Deliverables

- `Service-Manager-0.3.0-macOS-arm64.dmg`
- `service-manager-skill-0.3.0.zip`
- Versioned AI setup and skill installation guides in the task outputs directory.

## Limits

The new MCP tools were exercised with the real SDK on isolated fixtures, not an authenticated Codex/Claude model session or the user's live projects. Native folder-picker interaction was reused from the existing app and covered at the React/API boundary; installed-app UI/IPC was not exercised for this version. The installed app was not replaced or restarted. This release is not Developer ID signed/notarized. Folder grants limit project registration; executable commands continue to run with the user's permissions when explicitly trusted/granted and started.
