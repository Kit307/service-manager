# Service Manager 0.6.3 release verification

## Source gates
- Frontend: 81 tests in 13 files passed with one worker; ESLint and TypeScript/production build passed. This includes full sidebar Output, compact controls and later-pane keyboard focus.
- Backend: 51 full-suite tests passed, one opt-in Docker case ignored. No Rust behavior changed after that run; subsequent builds update release metadata. See TASK-0014 verification.
- Rust formatting and all desktop targets passed Clippy with warnings denied. Transitive block 0.1.6 reports future Rust incompatibility; current builds pass.
- Earlier concurrent runs under desktop load had fixture-loading timeouts. Focused and unchanged serial full suites passed without increasing timeouts.
- Source/installed Codex and Claude skills and MCP contract match byte-for-byte. Source skill YAML metadata and relative links were validated with Ruby YAML because the official Python quick_validate dependency PyYAML is unavailable.

## Native Glass
Uses the requested hkandala plugin through Rust, standard Clear, radius 16. Production and the standalone synthetic QA fixture share apply_window_glass. No React/WebGL displacement dependency was added; the alternative rdev library explicitly lacks Safari displacement.

Actual-source browser fixtures verified 11 tabs, five panes, drag/drop through gaps, keyboard controls, filters/follow, fixed pane actions and retained layout. Final console screenshots cover light/dark, 900px windows and forced 360px panes; production defaults to 440px panes. Sidebar clicks open full Output with Settings/History/Diagnostics; returning restores tabs without duplication.

The final isolated native fixture embeds production frontend assets and creates no Manager, services or credentials. On macOS 27.0.1 it reported native support and completed the Clear helper without fallback errors. It displayed the final console, opened full sidebar Output, restored its layout, added a third pane and moved a tab across panes through real pointer drag. Evidence: native-console-063.png, its full accessibility tree, and native-sidebar-full-output-063.png.

Native menu opening was observed; selection and the later-pane keyboard case use browser/unit proof rather than a claimed native selection pass. Independent review found and fixed lost queued log events, disappearing drag targets across gaps and stale pane focus. The later-pane regression failed before the fix and passed afterward.

## Release
Versioned DMG metadata and SHA256 are recorded in artifact.json after read-only mounting. Earlier 0.6.0–0.6.2 artifacts are preserved. The MIT dependency notice accompanies the app. Installed application, private projects and existing MCP clients remain untouched; only the installed instructions were updated.

## Limits
No external AI provider session, live Docker, older-macOS fallback runtime or Windows build was repeated. Native plugin uses private Apple APIs; future macOS/Rust compatibility needs revalidation. The DMG is not notarized. No remote push or installed-app replacement is performed.
