# Service Operations Implementation Plan

**Goal:** Complete the service-operations slice described in MILESTONE-0002.

**Architecture:** Keep process ownership and all lifecycle gates in Manager. Add small modules for port inspection and bounded history/diagnostic evidence; expose them through existing Tauri and MCP dispatch. React consumes those interfaces without launching processes itself.

**Tech Stack:** Existing Rust/Tokio/Serde/nix/reqwest, React/TypeScript, Tauri. Native macOS lsof/ps/open. Window material uses the Rust-only tauri-plugin-liquid-glass dependency pinned to 0.1.6; no React glass dependency.

## Global Constraints
- Preserve dirty work and real running services.
- Never read env-file values for diagnostics or put credentials into evidence.
- No remote push/deploy or installed-app replacement.

## Liquid Glass decision
Use [hkandala/tauri-plugin-liquid-glass](https://github.com/hkandala/tauri-plugin-liquid-glass) in the native layer. Do not add rdev/liquid-glass-react: its README documents Safari without displacement support, and macOS Tauri uses WKWebView. The selected plugin uses private APIs, creating OS compatibility risk; no App Store suitability is promised. The shared helper and final console were exercised on macOS 27.0.1 in an isolated native fixture. Older-macOS fallback remains untested.

## Tasks
- [x] Add defaulted optional service metadata and validation; verify old config compatibility.
- [x] Implement listener inspection and identity-checked termination; exercise held and released fixture ports.
- [x] Implement bounded private history and redacted diagnostic context with isolated storage tests.
- [x] Integrate history and dependencies into Manager; verify lifecycle and ordering with disposable processes.
- [x] Expose Tauri operations and read-only MCP history/diagnosis; retain current authorization.
- [x] Add settings, history/diagnostics UI, confirmation, group cancellation and actionable alerts; cover behavior with tests.
- [x] Integrate Rust-only tauri-plugin-liquid-glass 0.1.6 using standard native Clear on macOS 26+ (no experimental variant), with NSVisualEffect fallback on older macOS; verify readability and window behavior with an isolated native fixture.
- [x] Add persistent sidebar Background opacity (30–100%, default 55%, step 5), background-only CSS with reduced-transparency/high-contrast opaque overrides, and raised capsule rims/highlights/soft shadows on buttons/cards/tab bar; verify through v0.6.3 and preserve earlier artifacts.
- [x] Complete TASK-0016 Log workspace with no fixed pane/tab cap, drag and keyboard movement, per-tab session filters/follow, layout-only persistence and paused offscreen observers; closing a tab must never stop its service.
- [x] Update source skill/contract/docs, run focused and nearest full gates, build a versioned artifact; record focused source and documentation commits in git history.

Each task uses a failing behavior test before non-trivial implementation and is reviewed against the acceptance conditions before integration.
