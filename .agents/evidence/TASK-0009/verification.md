# TASK-0009 verification - v0.4.0

## Change
The product UI is Group → Service directly. Internal project/root identities remain unchanged for configuration, process ownership and MCP grants. Legacy private state defaults to no groups. All existing services initially appear ungrouped. No user files, installed app, real processes or client credentials were changed during verification.

## Passed
- `pnpm test --run`: 18 React tests in 5 files.
- `pnpm lint`: ESLint passed.
- `pnpm build`: TypeScript and Vite production build passed.
- `cargo test --all-targets`: 26 tests passed, 1 opt-in Docker test ignored. Includes native-feature compilation and real disposable lifecycle, loopback MCP/proxy and group persistence tests.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo fmt --check`: passed.
- Synthetic browser preview: group overview, scoped member list, collapse/expand and group settings inspected. At 1180 × 897 the document had no horizontal overflow. Preview and server were closed afterward.

## Preservation and failure cases
Rust tests check duplicate service IDs across roots, membership moves/deduplication, reload, group deletion retaining definitions, unchanged config bytes/runtime state, duplicate/invalid names and references, failed private-state writes preserving memory/disk, and cleanup when a service/root is explicitly removed. Existing tests cover legacy state and config, reviewed revisions, grants and revocation.
React tests check Run all skipping active services, Stop all restricted to the selected group, duplicate IDs outside the group remaining untouched, all reviews before lifecycle submission, cancellation on the second review, stale review failure and per-service restart errors. Groups do not create client grants or new lifecycle APIs.

## Limits
No new native interactive GUI check or real model-client session was run. Screenshot capture timed out; the browser's semantic snapshot and DOM viewport check do not certify visual layout. Live Docker integration was intentionally not rerun because Compose lifecycle implementation did not change. The release is not Developer ID signed or notarized. Browser tooling reported an Ego Lite update; it was not installed.

## Packaging
- `pnpm tauri build --bundles app`: release arm64 bundle succeeded (4m23s Rust release compilation).
- `hdiutil verify`: DMG checksum valid.
- Read-only mounted artifact verified v0.4.0, executable arm64 Mach-O, matching source icon bytes and Applications shortcut; exact mount detached successfully.
- Artifact receipt: `artifact.json`. SHA-256 `0c64616389fad78ddc7633b5c57fcfd28ee7455f654fafc113bd6302a72e7917`.
- Only the generated release app was unregistered and moved to Trash after packaging; build-folder Applications shortcut removed. Installed `/Applications/Service Manager.app` remains v0.3.0 and was not relaunched. No extra staged app copy was created.
- Deliverables: `outputs/Service-Manager-0.4.0-macOS-arm64.dmg` (8,084,489 bytes) and `outputs/Service-Manager-0.4.0-guide.md` outside the source repository.
