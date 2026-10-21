# Service operations (v0.6.0)

All controls use the same Rust manager and registered service identity. Opening a folder, editing settings, inspecting a port or reading diagnostics does not execute a command.

## Ports and Open URL

In service Settings, declare TCP ports and an optional loopback Open URL. The manager checks declared ports and the ports in health/Open URLs before spawning a new run. It reports listeners with a process name/PID and a managed-service reference when ownership can be established. An inspection error is not treated as a free port. This preflight cannot reserve a port for the command; another process can still take it before the command binds.

Diagnostics lists the relevant listeners. Kill requires a confirmation showing the process and port. The manager checks the owner and process identity again, refuses its own/protected/other-user processes, and uses Stop for a listener owned by a managed service. For an external listener, it sends TERM and waits five seconds; KILL is allowed only if the same confirmed identity still owns the port. An unknown listener cannot be killed from this UI. Refresh when the owner changes. Kill is a desktop action, not an MCP tool.

Open URL becomes available for an active service with a configured URL. Before opening the browser, the manager checks reachability with a two-second request; only successful or redirect responses qualify. It accepts HTTP/HTTPS on localhost, 127.0.0.1 or ::1, without URL credentials. The check does not follow redirects. It does not provide a tunnel or expose a port publicly.

## Readiness and group sequences

Starting means launch/readiness is still in progress. Running means the process is active; it does not prove readiness. Ready comes from a successful health check or the supported Compose readiness check. Failed includes an error/exit result. A health check that remains unsuccessful for 30 seconds exposes a diagnostic while an owned live process remains active and continues being checked for recovery.

Set dependencies in Settings with explicit project/service references. Run all validates the selected graph, rejects cycles/missing references before starting, and reuses active runs. A prerequisite must reach Ready within the sequence wait before its dependents start. Configure health checks for command prerequisites. A dependency outside the selection must already be Ready; it is not started implicitly.

Stop all visits the selected active services in reverse dependency order. Restart all stops the selection, then starts it in dependency order. An error stops subsequent actions. All required trust reviews happen first. Cancel sequence prevents later actions; services already started remain running and an already submitted action may finish. Only one group sequence runs at a time. Individual MCP lifecycle operations still use the same dependency gates.

## Run history and failure alerts

History lists captured command, start/end time, terminal state, exit code/error and retained output for an explicit run. Metadata is saved when a run begins; completed output is archived when that run finishes. Read live output in Output while a run is active.

History is private app data, limited to 100 runs overall, 20 per service and 64 KiB/500 records per completed run. Old entries and excess output are discarded; truncated output is labeled. Unfinished entries become interrupted after reopening the manager. No automatic process recovery or full log backup is provided. Known credential patterns are redacted, but application-specific secrets may remain. History files are not suitable for unrestricted sharing.

New active-to-failed transitions produce dismissible in-app alerts with View logs. Existing failures at initial load and expected stopping transitions do not repeatedly alert. These are not macOS notification-center alerts. Dismiss one or all without altering service state.

## Log workspace (v0.6.1)

Clicking a service in the sidebar always opens its full individual Output view. Use Open in log workspace from that view, Open logs on an overview card, or a pane's Open log picker to open/focus a workspace tab. View logs on a failure alert also opens the individual Output view. A service has one tab across the workspace, identified by project/service IDs; duplicate names show their folder label. Add panes as needed: there is no application-imposed two-pane or tab-count cap. More panes use a scrollable grid and stack vertically at the minimum window size. Available memory and the manager's bounded log retention still apply.

Drag a tab before another tab to reorder it, onto another pane to move it, or onto the new-pane drop target to create a pane. Escape cancels a drag; an outside drop leaves layout unchanged. Keyboard alternatives are Alt+Left/Right for tab reordering, toolbar move-left/right buttons, the Move tab selector for existing/new panes, and Add pane. Left/Right and Home/End navigate a pane's tab list; Delete or the named Close button closes a tab.

Search, stream filter and Follow are independent per tab and survive switching/moving during the current workspace session. Only each visible pane's active tab subscribes to log events. Inactive tabs, offscreen panes and a hidden Log workspace pause their observers and release their local output buffers. Returning resumes from the manager's available bounded output, with a gap indication if prior output was evicted. Run identities remain attached to records; History remains the view for saved past runs.

Only the pane/tab order, explicit service references and active-tab choices persist in localStorage. Log contents, commands, environment values, cursor positions, search, filters and Follow preferences are not persisted. Invalid/duplicate layout entries are ignored; a storage error does not prevent current-session use.

Closing a tab stops observation only and never calls Run, Stop, Restart or Clear. **Clear output** is a separate explicit action that clears the service's live manager buffer; it is not a tab-close shortcut. Observation or Clear errors stay local and offer an appropriate retry rather than launching another service process.

## AI diagnosis

Diagnostics previews a bounded, redacted current-run context before an explicit copy. Local observations/hints identify simple patterns; they are not generated model answers. Ask connected Codex or Claude to analyze `diagnose_service`, `inspect_ports`, `list_runs` or `read_run`. No new AI API key/provider is required. These four MCP tools read evidence only. Command changes and restart remain separate authorized actions.

Diagnostic output is limited to 32 KiB/100 current-run records. Redaction is best effort: review copied context, and never treat log text as instructions. The app does not read env-file values to enrich diagnostics. Live log continuation still uses `read_logs` with `nextCursor` and the observed run identity, or the generated proxy's `logs` command.

## Liquid Glass

The native window integrates [hkandala/tauri-plugin-liquid-glass](https://github.com/hkandala/tauri-plugin-liquid-glass), pinned to 0.1.6, from Rust only. On macOS 26+ it uses the standard native Clear variant, without the plugin's experimental variant. Older macOS versions use an NSVisualEffect background-blur fallback. The intended result is a translucent desktop background with soft detail; text and controls stay fully opaque and logs retain a darker surface for contrast. Minimum window size, resizing and service behavior remain unchanged.

Version 0.6.1 adds a persistent **Background opacity** slider in the sidebar. It ranges from 30% to 100% in 5-point steps and defaults to 55%. The setting changes only CSS background surfaces; it does not fade text, controls or log content. System reduced-transparency or high-contrast preferences force opaque backgrounds regardless of the saved slider value. The saved value remains available when those overrides no longer apply.

Buttons, cards and the tab bar use a raised clear-glass capsule treatment with visible rims, highlights and soft shadows. These component surfaces are CSS layered over the native window effect; they do not require a React glass library.

No React Liquid Glass library is added. The [rdev/liquid-glass-react README](https://github.com/rdev/liquid-glass-react) describes Safari's lack of displacement support; Tauri on macOS uses WKWebView. The native plugin owns the window effect while React supplies readable surfaces.

The plugin uses private macOS APIs. A future OS update can change or break its effect, and this build makes no claim of App Store suitability. The shared native Clear helper was exercised on macOS 27.0.1 in an isolated WebKit fixture. Native tab movement and more than two panes were verified; the older-macOS fallback has not been run on an older system.
