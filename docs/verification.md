# Service Manager 0.4.0 verification

- React: 18 behavior tests cover the direct Group → Service navigation, persisted disclosure, group creation/removal/error handling, scoped bulk controls, cancellation of the second trust review, stale reviewed revisions, partial lifecycle errors and existing editor/log/AI configuration behaviors.
- Rust: 26 passing tests cover group persistence, single membership across different roots with duplicate service IDs, preservation of configuration bytes and runtime snapshots, rejected members/names, failed-write rollback, membership cleanup, legacy state, and existing config/lifecycle/log/MCP boundaries. One opt-in Docker integration test remains ignored in this run.
- Frontend production build/typecheck, ESLint and Clippy with warnings denied passed. Final packaging and formatting evidence is in `.agents/evidence/TASK-0009/verification.md`.
- Browser preview used synthetic services only. Group selection, collapse/expand, settings and absence of horizontal document overflow at 1180 × 897 were checked. Screenshot capture timed out, so no new screenshot or native GUI behavior is certified for this version.
- No installed application replacement, real service restart, credential/grant change, model-session invocation, signing or notarization was performed. The existing native lifecycle/MCP integration tests use disposable fixtures. The previous native/real-client checks below are historical and were not repeated for v0.4.0.

# Service Manager 0.2.0 verification

## Implemented and exercised

- Real native release UI: Open Folder, Cancel, empty project, Add, Save, trust review, Run, stdout marker, Restart with a new PID, Stop, and reopen with the saved service definition. Light/dark themes were inspected and screenshots contain only the synthetic fixture. A separate 900 × 600 release shell was inspected.
- Rust: 16 automated tests cover strict config/path validation, persistence/conflicts/symlinks, reviewed-revision trust, real owned process groups, duplicate starts, stop escalation, shutdown admission, actual loopback readiness, a 30-second health diagnostic, failure/recovery, redirect refusal, cancellation after stop, bounded retention/cursors, a 20 MiB newline-free UTF-8 flood, authenticated MCP/proxy, grants and revocation.
- React: 6 behavior tests cover launch, retained logs on reselection, stale-draft revision during workspace events, safe service creation/removal, inert plain-text filtering, and one-time private Codex/Claude configuration reveal/hide.
- Real Docker opt-in integration: one uniquely named synthetic project using the already installed alpine image; duplicate start, ownership refusal for an existing unlabeled volume, stop during preflight, immutable launch config after editing the source, preserved stopped containers/volume, and persistent data after another run. Fixture containers/networks/volumes are cleaned explicitly by test teardown. Application Stop never removes them.
- Codex CLI 0.153.4: actual authenticated model session read the synthetic log marker through MCP. A second isolated invocation controlled the fixture and ran the generated proxy through its shell tool; it confirmed the same marker through both paths. Per-invocation settings were used; personal settings were not changed.
- Claude Code 2.1.181: executable found, but model authentication was unavailable. The user explicitly chose preparing configuration first. Copyable private snippets are now provided in AI Connections; no personal settings were modified.
- Independent source review found and resolved Compose early-stop publication, surviving plugin child, resource ownership and replica-status problems. Focused recheck found no remaining P1/P2 in that scope. This is not exhaustive security certification.

## Gates

Frontend tests, typecheck/build, ESLint, Rust tests, formatting, Clippy with warnings denied, live Docker fixture, and a release arm64 app build were run. Final artifact details and the native Quit/performance follow-up are recorded in `.agents/evidence/TASK-0002/verification.md`.

## Contract decisions

Lifecycle calls return a current run snapshot; the UI observes completion through status events. There is no separate operation-receipt persistence subsystem. Log gaps are represented by `truncated` and a continuation cursor. Events carry change notifications and bounded pages are fetched from the manager; they do not carry an unbounded stream of output. UI retained output has an additional 2 MiB cap.

Compose supports one long-running container per declared service. Unsupported scaling/profiles/container names/external resources/secrets/configs are rejected. Readiness requires a successful configured loopback check or Docker health checks for all expected containers. Stopped one-shot jobs are not treated as ready. Snapshot files can contain expanded environment values; they are private and removed when the run observer finishes.

## Boundaries

These results certify the exercised local macOS/Apple Silicon paths, not every scenario in the original planning matrix. Actual Claude model use awaits login. Codex desktop, Claude Desktop, cloud clients, exhaustive hostile-input/load matrices, and OS crash/daemonized-child recovery are not certified. No deployment, public publishing, signing identity, or notarization was performed.

Commands and reviewed Compose files run with normal user/Docker permissions. They are not sandboxed by this application. Existing user projects and stacks were excluded from tests. Abrupt OS/app termination can leave services behind; restart warns instead of killing or adopting arbitrary stored PIDs.
