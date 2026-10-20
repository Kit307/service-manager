# TASK-0012 - service panes and log severity

## Reproduction and source trace
- Before the fix, 15 regression cases failed: 12 log-tone cases lacked severity metadata, Settings removed the service heading/tabs and switching away lost both draft/filter state.
- LogPanel rendered `stderr` as `err`; CSS colored every stderr row red. Backend LogStore strips terminal escape codes but preserves the stdout/stderr channel. Thus informational output written to stderr looked like a failure. Synthetic INF/Success/status=200 on stderr reproduces this without any business data.
- App's Settings branch replaced the full service screen; Output tab was unavailable there.
- Native WebKit fixture reproduced HTML dragstart without a completed drop, both with and without Tauri's file handler. Disabling the handler alone did not prove a fix. Captured pointer movement was then used instead and the same native interaction completed successfully. No claim is made that the handler was the sole cause.

## Change and invariants
- Show explicit stdout/stderr/system badges. Recognized console prefixes (optionally after an ISO-style timestamp) and JSON string level drive only presentation: errors red, warnings amber, info/unknown neutral. Arbitrary error words and HTTP status are not parsed as severity. Other logger formats/numeric levels remain neutral.
- Raw messages, stream identity, server-side logs and MCP/proxy output remain unchanged.
- Output/Settings share the same heading/status/controls. Settings scrolls below the tabs; hidden Output stays mounted. A captured draft definition/revision survives tab switching and background refresh; Cancel/Save/target navigation ends it. Creation stays a standalone form.
- Service membership now uses captured pointer movement with a five-pixel threshold, hit testing of explicit group targets, click suppression after dragging, and Escape/pointer cancellation. External drag payloads are no longer accepted. Busy protection and backend validation/persistence stay intact. Native file handling remains at its default.
- macOS uses Overlay with hidden window title, native traffic lights, sidebar clearance and a top drag region with only the required window-drag capability. Edge resizing stays enabled and clamps at 900 × 600; Dock minimization remains disabled.

## Evidence
- 37 frontend tests in 6 files passed, including explicit stderr info, stdout error, warnings, malformed/unknown messages, unchanged stream filters, inert text, retained logs/search, stale revision preservation across Settings/Output, pointer membership changes, outside/cancelled drops, Escape, click selection and failed-save preservation.
- ESLint and TypeScript passed.
- Synthetic real-browser check: informational stderr and unstructured stderr used the same neutral computed color, explicit stdout error had a distinct red color, warning had amber. Stream badges stayed accurate.
- Browser measured identical service-header top/height across Settings, verified the settings pane starts below tabs and scrolls with overflow:auto, retained Restart/Stop, no horizontal document overflow, preserved a typed draft and log search across tab switches. No actual service operations were called. Browser task/server/temporary preview were closed/removed.
- Isolated native fixture, built from this Tauri context with a separate app identity and synthetic in-memory API: actual CUA pointer interaction moved QA One → Ungrouped → QA Two → QA One. A further check with the default native file handler also moved QA One → Ungrouped. No real command or manager/MCP instance ran.
- Native screenshot confirmed title-strip removal and retained traffic lights. Dragging the bottom-right edge toward a smaller size clamped the 1200 × 820 window at 900 × 600 (1800 × 1200 screenshot at 2x scale). Temporary native fixture sources/server/process were removed/stopped after verification.

## Limits
No real-project logs were read. Rust lifecycle/MCP/Docker/model-session tests were not rerun because process ownership and protocols did not change. The native app compiled for release; native interaction used a synthetic API fixture, while persistence remains covered by the prior Rust membership tests. The existing 0.5.0 token policy and installed client skills/config are unchanged. Installed app version 0.5.0 was read, not replaced or restarted. No signing/notarization.

## Release
Final release build passed with the new monochrome icon. DMG create/verify passed; a read-only mounted image confirmed version 0.5.1, arm64 executable, byte-identical ICNS and the Applications symlink. See artifact.json for SHA-256 and size. Temporary mount was detached.
