# TASK-0017 - Full-access MCP tokens

## Objective
Honor the explicit 2026-10-25 request for existing and new tokens to create/open projects, edit commands, read logs and control services without later folder grants.

## Scope
Shared MCP authorization and root discovery, AI Connections UI, compatibility tests, user documentation and bundled/installed skills; release 0.7.0. This supersedes only the parent-folder-grant restriction from TASK-0008 and TASK-0010; their other invariants remain.

## Non-Goals
No general file-delete/read tools, OS privilege bypass, service changes, credential rotation, remote/public endpoint or automatic deployment.

## Acceptance Criteria
- Old no-root/restricted tokens and new no-root tokens have the same effective filesystem-wide policy.
- Any existing absolute parent directory works; discovery lists filesystem roots.
- Relative traversal, invalid names, overwrite, out-of-request-root symlinks, malformed configs and revoked/unauthenticated requests remain rejected.
- Existing token values/profile data are preserved. Older binaries continue to interpret stored legacy roots on rollback.
- UI has no folder grant picker and clearly describes effective access and OS limits.
- Docs/skills distinguish v0.7 from older servers and never advise bypassing old-server rejection.

## Verification
Real SDK MCP create/open with disposable directories, legacy/new profiles and revocation; existing lifecycle/proxy integration; frontend tests/build/lint; native format/checks and app build.

## Stop Conditions
Do not restart the user's running manager/services merely to activate the update. Prepare the concrete build first and report any activation requirement.

## Risks
All valid tokens gain project filesystem access under the OS user, as explicitly requested. Keep token authentication, revocation, loopback-only binding, path validation and process ownership unchanged.

## Evidence Required
Test/build results and fresh package metadata; no credentials or real service logs.

## Completion Summary
Implementation complete: every valid existing/new token has full project creation/opening plus existing service controls. 82 frontend and 51 native tests passed; one opt-in Docker test ignored. Build, ESLint, fmt and strict desktop Clippy passed. DMG 0.7.0 built and verified read-only. Bundled and installed Codex/Claude skills match. Evidence: `.agents/evidence/TASK-0017/`. The Python skill validator lacked PyYAML; Ruby YAML frontmatter/reference checks passed. Installed-app activation is pending because Quit stops six active services; no running manager/service was changed. No remote push or publication.
