# TASK-0019 - OpenVPN authentication compatibility

## Objective
Support certificate-only profiles and encrypted inline keys so authorized local VPN setup can use existing OpenVPN exports.

## Scope
Optional private key passphrase, certificate-only UI, config-derived credential requirements, private askpass mount, narrow safe export directives, synthetic auth regression proof, 0.8.1 app/DMG.

## Non-Goals
Reading saved credentials from another app, decrypting or changing original private keys, exposing real configs/credentials in tests or commits, changing host VPN/routing, silently restarting current services.

## Dependencies
TASK-0018; existing Gluetun custom provider; users enter unavailable secrets directly in app.

## Acceptance Criteria
- No auth-user-pass means no username/password required or sent.
- auth-user-pass continues to require both values; certificate-only mode cannot bypass it.
- Encrypted keys use a private saved passphrase and trusted generated askpass path, never inline CLI values or public metadata.
- Original uploaded profiles stay untouched; unsupported scripts/files/management remain rejected.
- Synthetic encrypted certificate-only connection succeeds; wrong passphrase fails without reflecting secrets.

## Verification
Focused frontend/native tests, full frontend gates, desktop compile/release, existing userpass and new encrypted certificate-only Docker fixtures, independent review.

## Stop Conditions
Credentials unavailable: hand off secure app fields. Installed-app replacement requires a concrete update and approval because active services would stop.

## Risks
Private passphrases are stored mode0600 in app storage and mounted gateway-only. Docker-admin access remains trusted. OpenVPN provider compatibility requires real setup verification once credentials are entered.

## Evidence Required
Synthetic-only outputs and artifact versions. No uploaded key/certificate bodies or credential values in public evidence.

## Completion Summary
Software changes completed in 0.8.1: certificate-only login, private encrypted-key passphrases, narrow safe export compatibility, UI/API/docs and synthetic regression proof. 99 frontend tests, 70 native tests, two explicit VPN Docker fixtures and macOS release build passed. Independent review found no new P1/P2. The approved update was installed and the prior six active services restored.

Real setup remains an operational follow-up: a legacy server certificate required a separately approved profile-specific compatibility copy outside the repository; the second provider uses dynamic MFA/PIN and needs a new flow (TASK-0020). No customer configs, key material, PINs or credentials are committed.
