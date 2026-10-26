# TASK-0020 - Independent VPN MFA/PIN challenges

## Objective
Complete two independent simultaneous VPN connections by supporting dynamic CRV1 username/password then PIN challenges, keeping certificate-only profiles unchanged.

## Scope
Private authenticated OpenVPN management bridge inside the gateway namespace, per-profile challenge state and native response command, UI PIN form, no public management port, bounded cancellation/cleanup, synthetic real-server challenge proof, 0.8.2 app/DMG and authorized setup.

## Non-Goals
Host VPN/routing, credential/PIN collection in chat, PIN persistence or MCP mutations, guessing password+PIN concatenation, global certificate relaxation, SSO/CAPTCHA.

## Acceptance Criteria
- Initial account credentials and second-stage PIN follow actual CRV1 protocol with private nonce handling.
- Public challenge IDs are app-generated and profile-scoped; stale/cross-profile/duplicate replies fail safely.
- PIN values never enter config, history, logs, Docker args/env, public metadata or source artifacts.
- Pending authentication is visible and answerable while Connect is in progress.
- The management listener stays inside the owned gateway, authenticates with a private token inaccessible to workers, and gets no host publication.
- Timeouts/shutdown/restart release observers without leaking helpers or affecting the other profile.
- Existing username/password and certificate-only profiles retain behavior and ownership guards.

## Verification
Frontend/native gates, protocol-parser/quoting/bounds tests, production-manager CRV1 fixture with separate healthy profile, independent review, macOS release and configured two-profile status.

## Stop Conditions
Missing user response: hand off the local private PIN form. No security weakening for a provider without explicit authorization. Preserve current six services and certificate VPN until a concrete update is ready.

## Risks
Dynamic challenge implementations vary by provider. Response values are one-shot RAM inputs. Docker-admin access remains trusted.

## Evidence Required
Synthetic-only reproducible fixture results and artifact versions. No real profile bodies, endpoint identities, credentials, nonce or PIN values in public evidence.

## Completion Summary
Software complete in 0.8.2: independent per-profile CRV1 challenges, authenticated internal management bridge, private one-shot PIN input/cancellation, stale/cross-profile/replay rejection, authentication-cycle reset and safe legacy gateway migration. No additional application dependency.

Verification: 105 frontend tests, 76 native tests, desktop check/release build, independent final review (no remaining P1/P2), explicit real OpenVPN CRV1 synthetic fixture PASS in 34.72s and existing username/password sharing regression PASS in 214.43s. A concurrent-fixture Docker shutdown 15s timeout was not reproduced in sequential proof; timeout/ownership guards were not weakened.

macOS arm64 0.8.2 app and DMG built. The user approved activation and restoration, but the Mac is locked and the existing backend is not listening. The installed app remains 0.8.1; real MFA activation still awaits manual unlock and PIN input. Existing running services and the working certificate VPN were not changed by this pending update. See private activation notes for exact targets; no credentials or real profiles are in source artifacts.
