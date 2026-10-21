# TASK-0016 - Multiple live log tabs and drag layout

## Objective
Keep multiple service logs open and arrange an unlimited number of tabs and panes without starting another service process.

## Scope
A dedicated Logs workspace reuses ServiceRef, api.logs, api.onLogs, and LogPanel. Existing single-service Output, Settings, History, and Diagnostics remain available. No new dependencies or backend API changes.

## Interaction and Design
- Clicking a service in the sidebar always opens full individual Output. Open in log workspace or a pane picker opens or focuses its existing workspace tab. Each tab shows service/status, a folder label for duplicate names, and Close. Open log selectors contain registered services only.
- Drag a tab to reorder, append to another pane, or create a pane using the explicit new-pane drop target. Internal pointer capture, a five-pixel movement threshold, target hit testing, click suppression, Escape, and pointer cancellation follow the existing sidebar pattern. External drag data is ignored.
- Add pane creates another pane with no artificial count limit. Move left/right buttons, Alt+Arrow, and Move active log tab to pane provide keyboard equivalents. Tabs use Arrow/Home/End focus navigation and Delete to close. Explicit Close pane removes only that pane. Empty unrelated panes remain.
- Responsive CSS grid uses 440px minimum-width panes where space permits and stacks at narrow widths. The workspace scrolls for additional panes; log rows wrap to avoid page-level horizontal overflow.
- The console uses restrained tab underlines and a single compact search/stream/icon-action toolbar above higher-contrast log paper. Pane actions stay fixed while tabs scroll; default single-service Output remains unchanged. Reduced transparency/high contrast use the existing solid surface variables.
- Closing a tab or pane stops observation only. It never calls Run/Stop/Restart or clears logs. Clear output remains an explicit service-buffer mutation with a disabled pending state, local error, and stale-read invalidation after success.

## Architecture
LogWorkspace owns pane/tab metadata, active references, focus, per-tab search/filter/follow state, and last cursor. Pane content is observed through IntersectionObserver. Only the active tab of each visible pane mounts a feed; hiding the workspace or scrolling a pane out releases its frontend buffer and subscription. Unlimited metadata does not imply unlimited resident log buffers.

useServiceLogs extracts the existing single-flight cursor observer with event catch-up, 100ms bounded drain, sequence deduplication, 1,000-record/2MiB per-feed retention, cleanup, and stale response guards. Reactivation probes the previous cursor for actual server retention loss, then reloads the retained tail. The returned gap remains visible when the probe reports eviction. Log records retain their real runId. Historical runs remain in History.

Layout persistence stores only pane IDs, explicit service refs, tab order, and active refs. It validates duplicate/invalid shapes and tolerates storage errors. It never stores raw logs, commands, filters/search, env values, or diagnostic contents. Unavailable refs remain visible placeholders through initial asynchronous workspace loading and can be closed; no service registration or process action occurs.

## Files
- src/components/LogWorkspace.tsx: layout, tab/pane interactions, viewport gating, feed composition, explicit Clear output.
- src/lib/useServiceLogs.ts: bounded cursor observer, resume probe, retry/clear invalidation.
- src/styles/log-workspace.css: glass tabs, responsive panes, hidden guard, contrast overrides.
- src/test/LogWorkspace.test.tsx and src/test/useServiceLogs.test.tsx: focused lifecycle/layout proof.
- Root integration owns src/App.tsx; separate agent owns backward-compatible controlled LogPanel state and labels.

## Acceptance Criteria
1. More than eight tabs and more than two panes work; duplicate refs focus existing tabs.
2. Each tab retains its own filter/search/follow when switched or moved; only active in-viewport panes read logs.
3. Pointer reorder/new-pane drops and keyboard equivalents preserve explicit IDs and do not execute services.
4. Late responses after close or clear cannot restore stale rows. Cursor probes report actual retained-history loss.
5. Closing tabs preserves unrelated empty panes; initial services=[] does not discard restored refs.
6. Hidden Logs workspace is actually hidden, not just paused. Requested and clicked panes target toolbar actions correctly.
7. Native minimum-size, light/dark, reduced-transparency, focus, and pointer behavior remain usable.

## Verification
Ten focused Vitest tests cover independent search, observer-only close, unlimited restored tabs, additional panes, hidden/viewport pause, pointer reorder/new-pane/Escape, source-only pane cleanup/request focus, delayed close response, resume eviction, and clear stale-response invalidation, queued-event catch-up through empty responses, and new-pane target visibility across drag gaps. Run TypeScript and scoped ESLint after integration. Browser/native visual verification belongs to the root release pass.

## Stop Conditions
No floating native windows, new process ownership, or HTTP port forwarding is included. Ask before extending into these areas.

## Risks
Many simultaneously visible panes still consume proportionate bounded memory and reads. Offscreen panes reload manager-retained output rather than keeping a second unbounded history. Native pointer/visual behavior requires real WebKit verification beyond JSDOM.

## Evidence Required
Focused automated output and root browser/native screenshots at minimum and regular desktop sizes. Record any native behavior not checked.

## Completion Summary
Owned implementation complete; ten focused tests passed. Whole-app integration and native visual checks are handled by the root agent. No services started or stopped.
