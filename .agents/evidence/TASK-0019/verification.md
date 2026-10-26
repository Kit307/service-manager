# Authentication compatibility verification

- Frontend: 99 tests across 15 files passed; full lint/typecheck/Vite build passed.
- Native: 70 tests passed; three Docker fixtures opt-in by default.
- Explicit encrypted certificate-only Manager fixture: PASS111.11s, including wrong-passphrase rejection without outputting credentials.
- Explicit existing username/password shared gateway fixture: PASS47.48s.
- Release: macOS arm64 0.8.1 app built in1m55s; approved installed update and six-service restoration verified through desktop state.
- DMG: ../outputs/Service-Manager-0.8.1-macOS-arm64.dmg.
- SHA256: 2bb3aa31cd11ce7bdb04485df6aae890e6a979f8d3846a95eae56984462e7650.
- Original user files unchanged. Private operational copies remain outside the repository.
- Independent auth/parser/secret/migration review: no new P1/P2.

Dynamic two-stage PIN challenges are a distinct provider authentication flow and remain for the next approved task. This change never relaxes certificate verification globally.
