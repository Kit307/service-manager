# Project removal — 2026-10-20

## Implementation

- Added an accessible × control beside each sidebar project and a confirmation explaining that code/configuration files are preserved and unsaved edits are discarded.
- Active services disable the control. The shared ProjectStore also rejects starting/running/ready/stopping snapshots while holding the same Manager lock used to admit starts.
- Successful removal persists the workspace change, forgets service trust/Compose ownership records for the old project registration, and removes the in-memory project. Persistence failure restores private state and retains the project.
- Reopening the folder restores saved definitions under a new project identity, requiring fresh trust/AI grants. Compose containers/volumes are not deleted; the dialog and README explain that a reopened project creates a fresh managed stack rather than automatically reusing old Docker data.
- Selected-project removal chooses a remaining project or clears selection/output and shows the empty workspace. No MCP removal capability was added.

## Verification

- `pnpm test --run`: 11 tests passed, including confirmation/cancellation, persistence errors, active-service disabling, selection fallback, and Compose disclosure.
- `cargo test --manifest-path src-tauri/Cargo.toml --test persistence --test lifecycle`: 9 tests passed. Covered durable removal, unchanged code/config bytes, fresh registration/trust, all active states, persistence rollback, and rejecting removal while a real fixture process is running before accepting it after Stop.
- `pnpm lint`, Rustfmt check, Clippy (`--all-targets -- -D warnings`), and `git diff --check` passed.
- Final `pnpm tauri build --bundles app` passed, including TypeScript/Vite and the final confirmation copy. App bundle: 15.50 MiB; version 0.2.4.
- Created `Service-Manager-0.2.4-macOS-arm64.dmg` (8,055,827 bytes). Checksum verification passed; read-only mounted inspection confirmed version, exact ICNS bytes, executable permission, and Applications shortcut. The image was ejected.
- Moved only the two temporary packaging/build app bundles to recoverable Trash to avoid duplicate app icons. Kept the installed application and all previous deliverables.

## Limits

Native IPC/UI interaction was not exercised against the installed app; interaction tests and native lifecycle/persistence tests ran against isolated fixtures. The existing installed app was not replaced or restarted. This remains a local development build without Developer ID signing/notarization.
