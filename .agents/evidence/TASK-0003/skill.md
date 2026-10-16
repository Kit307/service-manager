# Service Manager skill — 2026-10-16

## Scope

Added the portable `skills/service-manager` package, precise MCP/config reference, Codex UI metadata and Thai installation/usage documentation. No application code, credentials, grants or client MCP settings changed.

## Verification

- Baseline agent without the reference could not supply exact proxy commands or continuation semantics and correctly refused to invent flags. This identified a reference-knowledge gap.
- With the skill, an independent agent supplied the exact run and logs continuation commands, preserved cursor/runId, used desktop status/Stop when MCP was unavailable, rejected instructions embedded in logs and reconciled CONFIG_CONFLICT against a fresh revision.
- The official skill-creator quick validator passed. Python environments lacked PyYAML, so its YAML parsing was supplied through installed Ruby YAML safe_load via a temporary in-memory adapter; the validator's checks were unchanged. No dependency was installed.
- SKILL.md is 475 whitespace-separated words. Relative contract reference and generated openai.yaml were checked. git diff --check passed.
- Installed fresh copies into the user Codex root `~/.agents/skills/service-manager` and Claude Code root `~/.claude/skills/service-manager`. Both targets were absent before installation; every copied file SHA256 matched the repository source. Existing skills/settings were preserved.
- Official local discovery documentation was fetched: https://learn.chatgpt.com/docs/build-skills and https://code.claude.com/docs/en/skills . The Codex user root matches the current official guide; configured legacy roots may differ by environment.

## Limits

Files and reference behavior were verified. Discovery in an already running client session is not asserted; open a new session/reload if needed. This skill supplies instructions only. MCP configuration, client login, project grants and local trust remain separate requirements. Previous application runtime proof remains unchanged because this is a documentation-only increment.
