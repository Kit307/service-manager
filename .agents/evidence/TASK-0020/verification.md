# Dynamic VPN MFA verification

- Frontend: 105 tests passed across 15 files. Full lint and TypeScript/Vite build passed.
- Native: 76 tests passed, including CRV1 parsing, command quoting, control-character rejection, base64 decoding, single-use/profile-scoped challenge IDs, authentication-cycle reset and profile-lock independence.
- Desktop check and macOS arm64 release build passed. The 0.8.2 bundle is 20.08 MiB; release compilation took 3m35s.
- Actual OpenVPN CRV1 synthetic fixture: PASS in 34.72s, run sequentially. A certificate VPN connects while another profile waits for MFA; a second MFA challenge can wait independently and be cancelled within 10 seconds. A correct PIN produces a healthy worker. Stale, cross-profile and replayed IDs are refused. PIN values do not appear in public metadata, Docker environment or logs. Shutdown and cleanup passed.
- Existing username/password sharing regression: PASS in 214.43s under concurrent load.
- An earlier concurrent-fixture Docker shutdown exceeded the existing 15-second capture limit. It did not reproduce in the sequential test. Ownership and timeout guards were retained.
- Final independent review found no remaining P1/P2 after the profile-specific controls, cancellation and authentication-cycle fixes.
- DMG: ../outputs/Service-Manager-0.8.2-macOS-arm64.dmg.
- SHA256: 106bc43c73fc28eaa280d86739b40c10b19d371057153c6837b233a7af650727.
- The DMG checksum is valid. The app is not Developer ID signed or notarized.

Real provider activation still requires manual screen unlock and private PIN entry. No automated unlock or force quit was attempted. No real VPN configuration, credential, PIN, server nonce or endpoint identity is committed. Reproducible synthetic scripts are recorded under TASK-0018 evidence.
