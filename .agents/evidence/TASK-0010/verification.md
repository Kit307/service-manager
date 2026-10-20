# Token-wide access verification

- Red: the real-SDK/proxy test failed creating an empty-scope token with `CONFIG_INVALID: Choose a client name and at least one project or allowed folder.` before the change.
- Shared path: middleware verifies bearer token/Origin and enabled state; every tool checks current profile existence, then registered-target/revision/path/process checks run in the existing manager. No extra manager was created.
- All currently valid tokens, including legacy profiles, intentionally adopt full workspace read/control/edit semantics approved by the user. Legacy scope arguments/serialized fields stay readable for compatibility; no real token file was edited by this session.
- Token creation requires no selected project/root. A folder registered after connection is visible to the same token; MCP can read/upsert there. MCP start/restart authorizes the current registered definition, and the manager still rejects replacement after trust. Proxy and native share the same owned run and logs.
- Root creation/opening still checks the profile's allowed parent folders, traversal/symlinks/existing paths. Registration no longer writes redundant project grants into token storage.
- Existing tests cover malformed/stale revisions, invalid targets, wrong token, absent Authorization, browser Origin and revocation. UI creation works without any project/permission checkboxes. Full-workspace command-execution implications are displayed in the app.
- Updated source skill passed quick_validate. No paid model sessions or personal Codex/Claude config writes were performed.

Frontend 21 tests passed, ESLint/typecheck passed; Rust full suite 27 passed with 1 ignored Docker case, relevant suites rechecked after changes. Clippy passed with warnings denied. Release artifact evidence follows in TASK-0011.
