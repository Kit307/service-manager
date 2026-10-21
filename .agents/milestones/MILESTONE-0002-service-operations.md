# MILESTONE-0002 - Service operations and diagnosis

## Objective
Deliver configured-port preflight and confirmed kill, verified local URL opening, dependency-aware group startup, bounded run history, actionable failure alerts, and AI diagnosis through the existing MCP clients, with a macOS Liquid Glass interface and blurred desktop translucency.

## Scope
Extend the existing shared manager; preserve current configurations, trust, process ownership and valid tokens. macOS remains the supported native target. Use real lifecycle fixtures rather than touching user services.

## Decisions
- Optional ports, loopback URL and explicit project/service dependencies. Empty new fields serialize away to preserve existing fingerprints.
- Port conflict information includes PID/name, identity evidence and managed-service identity. Kill is a desktop action with confirmation and revalidation; managed processes use manager Stop. Never kill arbitrary process groups or a reused PID.
- Readiness reuses existing health checks. Group dependencies require verified Ready; reject cycles and report missing/unhealthy dependencies. Cancel prevents future starts without stopping already-started services.
- History stays in private app data, bounded by run count and log size, with started/finished timestamps and per-run output. Failures produce dismissible in-app alerts linked to the service log.
- AI clients use a new read-only diagnostic bundle and history tools. UI previews/copies a bounded, redacted context for those clients; heuristic observations are labeled, not presented as generated AI answers.
- Use Rust-only tauri-plugin-liquid-glass 0.1.6 with standard native Clear on macOS 26+ (no experimental variant) and NSVisualEffect fallback on older macOS. Keep readable log surfaces, minimum sizing and opaque text. No React glass library: rdev/liquid-glass-react documents Safari without displacement support, and Tauri uses WKWebView. The plugin uses private APIs; OS compatibility and App Store suitability are not guaranteed. The native Clear helper was exercised on macOS 27.0.1; the older-macOS fallback remains untested.
- Release v0.6.3 includes a persistent sidebar Background opacity slider: 30–100%, default 55%, step 5. Change background CSS only, preserve text/log opacity, and force opaque backgrounds for reduced transparency/high contrast. Buttons/cards/tab bar use CSS raised clear-glass capsule rims, highlights and soft shadows. Preserve the already-built v0.6.0–0.6.2 artifacts.
- Include TASK-0016 Log workspace in v0.6.1: any number of panes/tabs without a fixed two-pane cap, pointer/keyboard reorder and move, session-local per-tab filters/follow, layout-only persistence and paused hidden/offscreen observers. Closing a tab never changes service lifecycle.
- No public tunnel, hosted AI provider, automatic command repair, automatic installation/restart or remote publication.

## Tasks
- TASK-0014: Shared backend operations and regression tests.
- TASK-0015: UI, MCP, documentation and integrated verification.
- TASK-0016: Multiple log tabs/panes, observer lifecycle and accessible layout controls.

## Completion Criteria
Every requested path has a usable UI/API, meaningful regression coverage and a passing frontend/native gate. Report native/Docker/provider checks that were not performed. Produce a versioned local app artifact after the source checks pass.

## Completion
Completed through v0.6.3. Frontend 81 tests and Rust 51 tests pass; one opt-in Docker case is ignored. Browser/native UI proof and read-only DMG validation are in TASK-0015 evidence. Installed services and credentials remain untouched; older-macOS fallback, provider sessions, Windows and live Docker were not repeated.
