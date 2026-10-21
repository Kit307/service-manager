# TASK-0015 - Service operations UI and AI access

## Objective
Expose operations through desktop UI and read-only MCP evidence tools.

## Acceptance Criteria
Settings preserve ports/URL/dependencies. History and Diagnostics remain below the service shell. Kill requires a visible confirmation. Group cancellation is usable while a sequence waits. Failure alerts are dismissible and link to Output. AI context previews before copy and diagnostics never execute suggested repairs. The native window uses Rust-only tauri-plugin-liquid-glass 0.1.6: standard native Clear on macOS 26+, no experimental variant, NSVisualEffect fallback on older macOS. Text/logs remain readable, and minimum size/controls remain usable. Do not add a React glass library. The sidebar Background opacity slider persists 30–100%, default 55%, step 5, changing only background CSS; text/log opacity stays unchanged. Reduced transparency/high contrast force opaque backgrounds. Buttons/cards/tab bar show raised clear-glass capsule rims, highlights and soft shadows through CSS. Deliver v0.6.3 with the redesigned console and full sidebar Output navigation, preserving the v0.6.0–0.6.2 artifacts.

## Verification
Cover slider bounds/default/persistence and background-only opacity; inspect accessibility overrides and raised-surface readability. Frontend behavior tests, TypeScript, ESLint, production build; Rust MCP regression tests and isolated browser/native fixture as available. The native Clear helper was exercised on macOS 27.0.1; final console QA is recorded separately, and the older-macOS fallback remains untested. Verify macOS 26+ Clear and older-macOS fallback separately from CSS/browser appearance. The plugin uses private APIs, so future OS compatibility and App Store suitability are not promised. No real user service restart or secret-file reads.

## Completion Summary
Complete. Frontend 81/81 tests, build/typecheck/ESLint, Rust 51 tests plus one opt-in Docker ignored, formatting and strict desktop Clippy passed. Actual-source browser and isolated macOS 27.0.1 native proof cover the final console, full sidebar Output, retained tabs, additional panes and pointer movement. The 0.6.3 arm64 DMG was checksum-verified and read-only mounted; version, icon, license and Applications link match. See TASK-0015 artifact.json and verification.md. Earlier artifacts, installed app/services and credentials were preserved. Temporary QA bundles were closed, unregistered and archived. No remote publication, signing or notarization.

Source commits: f911c92 (native platform), d735546 (React UI). Documentation and verification are recorded in the following documentation commit.
