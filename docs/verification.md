# Service Manager 0.5.1 verification

37 frontend tests passed, plus ESLint and TypeScript. Informational stderr is neutral and explicit recognized error/warning levels determine color; raw streams/text remain unchanged. Real Chromium checks verified computed colors and shared service-heading bounds, Settings below the tabs, retained draft/log search and internal scrolling. Creation and stale revision protection remain covered. Isolated native WebKit pointer checks verified moving a service out of a group and into another, including direct cross-group movement. Native titlebar overlay retains traffic lights; resizing clamps at 900 × 600. See `.agents/evidence/TASK-0012/` for details and release evidence.

No backend lifecycle/authorization behavior changed. Rust runtime/Docker/model-session tests were not repeated; native packaging is compiled during release. The GUI fixture uses synthetic API state; real services, installed app and client credentials remain untouched. All previous verification sections below are historical.

# Service Manager 0.5.0 verification

- Frontend: 21 tests in 5 files passed, including token setup without scope selection, typed drag/drop and persistence errors, icon action mapping, existing group/bulk/editor/log behavior. Typecheck/build and ESLint passed.
- Native: 27 Rust tests passed; one opt-in Docker case remains ignored. Coverage includes legacy and new token-wide access, later registration, MCP upserts/stale revisions, start/restart and proxy shared-run logs, bad tokens/revocation/Origin, filesystem root containment and atomic membership moves preserving config/runtime. Formatting and Clippy with warnings denied passed.
- Real pointer drag/drop was exercised in Chromium using a synthetic API fixture: Running service moved out to Ungrouped and into an empty group. Native persistence is verified separately by Rust tests; native WebKit pointer interaction is not certified for this version.
- Existing valid tokens intentionally gain full read/control/command-edit access for every registered folder after upgrade, as explicitly selected by the user. No personal credential files, installed app or real service processes were changed during verification.
- Source skill quick_validate passed. Paid model-client sessions, live Docker and native interactive GUI tests were not repeated. Release/DMG evidence is in `.agents/evidence/TASK-0011/`.

The following sections are historical verification of earlier versions and their scoped-token policy.

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
