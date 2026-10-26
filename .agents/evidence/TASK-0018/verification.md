# TASK-0018 verification

Synthetic, local-only checks; no organization VPN profiles, credentials, installed-app replacement or current service lifecycle changes.

- Frontend: `pnpm test --run`: 96 tests passed across 15 files.
- Frontend: full ESLint and TypeScript/Vite production build passed.
- Native: `cargo test --no-default-features`: 68 tests passed; opt-in Docker Compose fixture ignored by default.
- Docker binding unit test: passed; persisted socket/daemon replacement and symlink refusal checked.
- Native desktop: `cargo check --all-targets` passed. Existing dependency `block 0.1.6` emits a future-Rust compatibility notice.
- Existing Docker Compose regression: explicit ignored fixture passed, 23.42 seconds; uniquely named resources cleaned.
- Independent code review: initial daemon binding, blocking event polling and unhealthy-disconnect issues resolved and rereviewed. Post-crash user-list limitation documented; actual namespace checks remain authoritative.

Final native rerun: 68 passed, 2 Docker fixtures ignored by default. The VPN fixture was separately run explicitly and passed in 37.30 seconds. The final added remote normalization/import tests pass. macOS arm64 release build passed in 1m56s; app bundle version verified as 0.8.0.

Targeted final review found no new parser/credential issue. Existing gateway remote-IP snapshots do not follow later DNS rotation; remove/reimport recovery is documented. This release does not recreate an existing gateway automatically.

Artifacts:
- App: src-tauri/target/release/bundle/macos/Service Manager.app (19.97 MiB), arm64, bundle version 0.8.0.
- DMG: ../outputs/Service-Manager-0.8.0-macOS-arm64.dmg.
- SHA256: 3143652958bfc014473cfb64714aab26d38719d1c1597a42317217829a8541aa.
- Unsigned/not notarized; installed 0.7.0 app and its services were not replaced or restarted.
- Public Gluetun image downloaded for synthetic checks remains available locally; all fixture containers, networks and temporary key folders were cleaned.
