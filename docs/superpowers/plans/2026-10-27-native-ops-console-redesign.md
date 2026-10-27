# Native Ops Console Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current visual hierarchy with the approved Native Ops Console design while preserving every existing service, group, log, VPN, AI, and lifecycle behavior.

**Architecture:** Keep all workspace state and backend calls in their current React components. Reshape only semantic markup and the existing CSS layers, with behavior tests guarding lifecycle and interaction contracts. Validate the result in the isolated Tauri Test app before any production bundle or installation.

**Tech Stack:** React 19, TypeScript, CSS, Vitest, Testing Library, Tauri 2, Rust.

## Global Constraints

- Keep the existing 900×600 minimum window size and prevent horizontal page scrolling at that size.
- Preserve Rust service lifecycle, project/config schema, MCP tools and ports, VPN behavior, log retention, and redaction.
- Do not add a frontend dependency, web font, raster icon, or backend endpoint.
- Use macOS system sans-serif and the existing SF Mono-compatible monospace stack.
- Keep visible focus, keyboard alternatives for drag actions, status text beyond color, and `prefers-reduced-motion` support.
- Keep reading-critical log, form, alert, and command surfaces legible at every background-opacity setting.
- Do not build, install, replace, restart, or modify the production application or its registered services.

---

### Task 1: Native Ops visual foundation and shell

**Files:**
- Modify: `src/styles.css:1-465`
- Modify: `src/styles/glass-surfaces.css`
- Modify: `src/App.tsx:427-620`
- Test: `src/test/Appearance.test.tsx`
- Test: `src/test/App.test.tsx`

**Interfaces:**
- Consumes: existing `theme`, `backgroundOpacity`, `runningCount`, `view`, `workspace`, and `error` state from `App`.
- Produces: semantic shell classes `ops-shell`, `ops-sidebar`, `ops-workspace`, `ops-context-bar`, and shared tokens used by later tasks.

- [ ] **Step 1: Add failing shell semantics tests**

Add assertions to the existing app tests without changing their fixtures:

```tsx
expect(document.querySelector(".app-shell")).toHaveClass("ops-shell");
expect(screen.getByRole("complementary", { name: "Services" })).toHaveClass("ops-sidebar");
expect(screen.getByRole("main")).toHaveClass("ops-workspace");
expect(screen.getByRole("banner", { name: "Workspace context" })).toBeVisible();
```

- [ ] **Step 2: Run the focused tests and confirm RED**

Run:

```bash
rtk pnpm test -- --run src/test/App.test.tsx src/test/Appearance.test.tsx
```

Expected: FAIL because the Native Ops classes and named banner do not exist.

- [ ] **Step 3: Define the token system in `src/styles.css`**

Replace the current visual values, not their semantic names, and add only the tokens required by the approved design:

```css
:root {
  --ops-canvas: #e4e9ef;
  --ops-panel: rgb(248 250 252 / 78%);
  --ops-panel-solid: #f4f7fa;
  --ops-console: #0c1117;
  --ops-console-ink: #dbe4ee;
  --ops-rim: rgb(69 83 101 / 22%);
  --ops-focus: #1d4ed8;
  --ops-motion-fast: 120ms;
  --ops-motion-base: 180ms;
  --ops-radius-sm: 8px;
  --ops-radius-md: 12px;
}

:root[data-theme="dark"] {
  --ops-canvas: #11161d;
  --ops-panel: rgb(24 31 40 / 82%);
  --ops-panel-solid: #171e27;
  --ops-console: #090d12;
  --ops-console-ink: #d9e2ec;
  --ops-rim: rgb(204 216 230 / 16%);
  --ops-focus: #93c5fd;
}
```

Map the existing `--canvas`, `--bg`, `--surface`, `--line`, `--ink`, and status tokens to these values so feature CSS files continue using one source of truth. Keep automatic dark mode equivalent to the explicit dark mapping.

- [ ] **Step 4: Reshape the shell without changing state flow**

Add the new classes to existing elements and name the context header:

```tsx
<div className={`app-shell ops-shell${macClass}`}>
  <aside className="sidebar ops-sidebar" aria-label="Services">…</aside>
  <main className="workspace ops-workspace">
    …
    <header className="workspace-bar ops-context-bar" role="banner" aria-label="Workspace context">…</header>
```

Do not move `FailureAlerts`, sequence status, global error, or unclean-shutdown warning outside the main workspace.

- [ ] **Step 5: Implement shell layout, focus, glass, and reduced motion**

Use the existing 232px sidebar as the initial width, preserve the fixed macOS drag region, and ensure reading surfaces use `--ops-panel-solid` or `--ops-console` instead of the adjustable background alpha. Add:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
  }
}
```

- [ ] **Step 6: Run focused tests and compile**

Run:

```bash
rtk pnpm test -- --run src/test/App.test.tsx src/test/Appearance.test.tsx
rtk pnpm typecheck
```

Expected: all selected tests PASS and TypeScript exits 0.

- [ ] **Step 7: Commit the shell deliverable**

```bash
rtk git add src/App.tsx src/styles.css src/styles/glass-surfaces.css src/test/App.test.tsx src/test/Appearance.test.tsx
rtk git commit -m "feat(ui): establish native ops shell"
```

---

### Task 2: Operational sidebar and group overview

**Files:**
- Modify: `src/components/ServiceSidebar.tsx`
- Modify: `src/components/GroupOverview.tsx`
- Modify: `src/styles.css:190-403`
- Modify: `src/styles/overview.css`
- Test: `src/test/Groups.test.tsx`
- Test: `src/test/GroupOverview.test.tsx`

**Interfaces:**
- Consumes: existing `ServiceEntry`, `WorkspaceGroup`, drag/drop callbacks, group actions, and bulk-control callbacks.
- Produces: status-readable compact service rows and an operational list overview; callback signatures stay unchanged.

- [ ] **Step 1: Add failing status and operation-list tests**

Extend existing tests with user-visible behavior:

```tsx
expect(screen.getByRole("button", { name: /Merchant Portal.*Ready/i })).toBeVisible();
expect(screen.getByRole("region", { name: "Group services" })).toBeVisible();
expect(screen.getByRole("button", { name: "Run all services" })).toBeVisible();
expect(screen.getByRole("button", { name: "Stop all services" })).toBeVisible();
```

Keep the current collapse-memory, group move, and drop assertions unchanged.

- [ ] **Step 2: Run focused tests and confirm RED**

```bash
rtk pnpm test -- --run src/test/Groups.test.tsx src/test/GroupOverview.test.tsx
```

Expected: new semantic names/regions fail; existing behavior tests continue passing.

- [ ] **Step 3: Make service state readable without color**

Keep each service row a native button. Its accessible name must contain the service name and label from the existing state map. Render one visual status dot plus one short state label; do not add a second state source.

```tsx
<button aria-label={`${service.name} · ${stateLabel}`} …>
  <span className={`status-dot ${snapshot.state}`} aria-hidden="true" />
  <span className="service-name">{service.name}</span>
  <span className="service-state">{stateLabel}</span>
</button>
```

- [ ] **Step 4: Restyle sidebar hierarchy**

Keep group collapse, edit, count, drag handle, selection, utility navigation, opacity, appearance, and running count. Use a 4/8px spacing rhythm, 32–36px rows, at least 3:1 control boundary contrast, and `text-overflow: ellipsis` for long names. Do not hide the only action on hover.

- [ ] **Step 5: Convert overview cards into an operational list**

Add `aria-label="Group services"` to the service collection. Keep dependencies, state, per-service action, sequence output, and empty state. Change bulk action accessible names to `Run all services`, `Restart all services`, and `Stop all services` while keeping callbacks and confirmations unchanged.

- [ ] **Step 6: Run focused tests and verify keyboard behavior**

```bash
rtk pnpm test -- --run src/test/Groups.test.tsx src/test/GroupOverview.test.tsx src/test/workspace.test.tsx
```

Expected: all selected tests PASS, including existing drag and group persistence tests.

- [ ] **Step 7: Commit the navigation deliverable**

```bash
rtk git add src/components/ServiceSidebar.tsx src/components/GroupOverview.tsx src/styles.css src/styles/overview.css src/test/Groups.test.tsx src/test/GroupOverview.test.tsx
rtk git commit -m "feat(ui): clarify service navigation"
```

---

### Task 3: Service operation header and consistent forms

**Files:**
- Modify: `src/App.tsx:620-845`
- Modify: `src/components/ServiceEditor.tsx`
- Modify: `src/components/GroupEditor.tsx`
- Modify: `src/components/VpnSettings.tsx`
- Modify: `src/components/AiConnections.tsx`
- Modify: `src/components/ServiceOperations.tsx`
- Modify: `src/styles.css:465-1377`
- Modify: `src/styles/operations.css`
- Modify: `src/styles/vpn.css`
- Test: `src/test/OperationsIntegration.test.tsx`
- Test: `src/test/ServiceOperations.test.tsx`
- Test: `src/test/VpnSettings.test.tsx`
- Test: `src/test/AiProjects.test.tsx`

**Interfaces:**
- Consumes: existing service snapshot, `control`, `beginEdit`, save/remove callbacks, VPN API, AI API, and operations API.
- Produces: stable `service-operation-header`, `service-command-beam`, `service-view-tabs`, and shared `form-section`/`form-actions` presentation contracts.

- [ ] **Step 1: Add failing service-header and form semantics tests**

Add assertions to existing behavior tests:

```tsx
expect(screen.getByRole("region", { name: "Service controls" })).toBeVisible();
expect(screen.getByRole("group", { name: "Service command and status" })).toBeVisible();
expect(screen.getByRole("tablist", { name: "Service views" })).toBeVisible();
expect(screen.getAllByRole("tab").filter((tab) => tab.getAttribute("aria-selected") === "true")).toHaveLength(1);
```

For VPN and AI forms:

```tsx
expect(screen.getByRole("region", { name: "Connection settings" })).toBeVisible();
expect(screen.getByRole("group", { name: "Form actions" })).toBeVisible();
```

- [ ] **Step 2: Run focused tests and confirm RED**

```bash
rtk pnpm test -- --run src/test/OperationsIntegration.test.tsx src/test/ServiceOperations.test.tsx src/test/VpnSettings.test.tsx src/test/AiProjects.test.tsx
```

Expected: semantic regions, tabs, and action groups are missing.

- [ ] **Step 3: Build the stable service operation header**

Wrap the existing service summary with `role="region" aria-label="Service controls"`. Keep every action condition and callback unchanged. Apply these rules:

- stopped/failed: `Run service` is primary;
- active: `Restart` is secondary and `Stop` is destructive;
- starting/stopping: retain exact disabled logic and progress copy;
- URL action remains conditional on existing reachability state.

Convert the existing command row into:

```tsx
<div className="command-display service-command-beam" role="group" aria-label="Service command and status">
  …existing command…
  …existing edit button…
</div>
```

- [ ] **Step 4: Give the view rail tab semantics**

Use the existing view state and click handlers:

```tsx
<div className="service-tabs service-view-tabs" role="tablist" aria-label="Service views">
  <button role="tab" aria-selected={view === "service"}>Output</button>
  <button role="tab" aria-selected={view === "settings"}>Settings</button>
  <button role="tab" aria-selected={view === "history"}>History</button>
  <button role="tab" aria-selected={view === "diagnostics"}>Diagnostics</button>
</div>
```

The separate `Open in log workspace` control remains a button, not a tab.

- [ ] **Step 5: Apply one form grammar**

Add presentation-only wrappers to the existing editors:

```tsx
<section className="form-section" aria-labelledby="connection-settings-heading">
  <h2 id="connection-settings-heading">Connection settings</h2>
  …existing labeled fields and helper text…
</section>
<div className="form-actions" role="group" aria-label="Form actions">…existing actions…</div>
```

Do not rename API arguments, clear user input on failure, expose secrets, or move field errors away from their fields.

- [ ] **Step 6: Restyle history, diagnostics, VPN, and AI surfaces**

Use the shared panel, field, action, and error tokens. Keep destructive actions separated, use a stable maximum reading width for forms, and preserve the full-width log/history output where present.

- [ ] **Step 7: Run focused tests and typecheck**

```bash
rtk pnpm test -- --run src/test/OperationsIntegration.test.tsx src/test/ServiceOperations.test.tsx src/test/VpnSettings.test.tsx src/test/AiProjects.test.tsx
rtk pnpm typecheck
```

Expected: all selected tests PASS and TypeScript exits 0.

- [ ] **Step 8: Commit the service/forms deliverable**

```bash
rtk git add src/App.tsx src/components/ServiceEditor.tsx src/components/GroupEditor.tsx src/components/VpnSettings.tsx src/components/AiConnections.tsx src/components/ServiceOperations.tsx src/styles.css src/styles/operations.css src/styles/vpn.css src/test/OperationsIntegration.test.tsx src/test/ServiceOperations.test.tsx src/test/VpnSettings.test.tsx src/test/AiProjects.test.tsx
rtk git commit -m "feat(ui): unify service operation views"
```

---

### Task 4: Unified log console and pane accessibility

**Files:**
- Modify: `src/components/LogPanel.tsx`
- Modify: `src/components/LogWorkspace.tsx`
- Modify: `src/styles/log-panel.css`
- Modify: `src/styles/log-workspace.css`
- Modify: `src/styles.css` only for shared console tokens
- Test: `src/test/LogPanelState.test.tsx`
- Test: `src/test/LogSeverity.test.tsx`
- Test: `src/test/LogWorkspace.test.tsx`
- Test: `src/test/OperationsIntegration.test.tsx`

**Interfaces:**
- Consumes: existing log records, cursors, pane persistence, drag state, filter/search/follow state, and callbacks.
- Produces: common `ops-console`, accessible log tablist/tab semantics, and unchanged pane persistence format.

- [ ] **Step 1: Add failing log semantics tests**

Extend existing tests:

```tsx
expect(screen.getByRole("region", { name: "Service logs" })).toHaveClass("ops-console");
expect(screen.getAllByRole("tablist", { name: /Log tabs/i }).length).toBeGreaterThan(0);
expect(screen.getByRole("button", { name: "Move active log tab left" })).toBeVisible();
expect(screen.getByRole("button", { name: "Move active log tab right" })).toBeVisible();
expect(screen.getByLabelText("Move active log tab to pane")).toBeVisible();
```

Keep existing assertions for unlimited restored metadata, unavailable references, viewport observation, drag threshold, and persisted panes.

- [ ] **Step 2: Run focused tests and confirm RED**

```bash
rtk pnpm test -- --run src/test/LogPanelState.test.tsx src/test/LogSeverity.test.tsx src/test/LogWorkspace.test.tsx
```

Expected: the shared console class and tab semantics fail; existing log behavior remains green.

- [ ] **Step 3: Apply the shared console structure**

Add `ops-console` to both compact service output and workspace panes. Use `role="tablist"` on each pane tab strip and `role="tab" aria-selected={active}` on its tab buttons. Keep close buttons separate so closing a tab never activates it first.

- [ ] **Step 4: Preserve drag alternatives and status meaning**

Keep the existing move-left, move-right, pane selector, add-pane control, and Escape cancellation. Icons remain decorative beside visible text. Stream and severity labels remain visible, so color is never the only signal.

- [ ] **Step 5: Restyle the console for both themes**

The log surface always uses the opaque `--ops-console` and `--ops-console-ink` tokens. Timestamp, stream, severity, search, filters, follow, copy, clear, gap, and receiving/bounded-history status remain readable. Use `font-variant-numeric: tabular-nums` for timestamps and status counts.

- [ ] **Step 6: Run log and integration tests**

```bash
rtk pnpm test -- --run src/test/LogPanelState.test.tsx src/test/LogSeverity.test.tsx src/test/LogWorkspace.test.tsx src/test/OperationsIntegration.test.tsx
```

Expected: all selected tests PASS and the existing pane persistence assertions are unchanged.

- [ ] **Step 7: Commit the log deliverable**

```bash
rtk git add src/components/LogPanel.tsx src/components/LogWorkspace.tsx src/styles/log-panel.css src/styles/log-workspace.css src/styles.css src/test/LogPanelState.test.tsx src/test/LogSeverity.test.tsx src/test/LogWorkspace.test.tsx src/test/OperationsIntegration.test.tsx
rtk git commit -m "feat(ui): unify service log consoles"
```

---

### Task 5: Full regression and isolated desktop acceptance

**Files:**
- Modify: `docs/superpowers/specs/2026-10-27-native-ops-console-redesign-design.md` only if verified behavior requires clarification
- Create: `.agents/evidence/TASK-0025/native-ops-console-smoke.md`
- Test: all existing frontend and Rust tests

**Interfaces:**
- Consumes: the completed visual deliverables and existing `pnpm test-app` isolated runtime.
- Produces: bounded verification evidence; no production bundle or installation.

- [ ] **Step 1: Run frontend regression sequentially**

```bash
rtk pnpm test -- --run
rtk pnpm lint
rtk pnpm build
```

Expected: 15 test files and 105 or more tests PASS, ESLint exits 0, and TypeScript/Vite production compilation exits 0. Record the actual counts rather than copying this lower bound into evidence.

- [ ] **Step 2: Run Rust regression outside the socket-restricted sandbox when required**

```bash
rtk run '"${CARGO_HOME:-$HOME/.cargo}/bin/cargo" test --all-targets'
```

Expected: all non-ignored Rust tests PASS. Keep the explicit Docker tests ignored in this command.

- [ ] **Step 3: Run the synthetic MCP/VPN proof**

```bash
rtk run '"${CARGO_HOME:-$HOME/.cargo}/bin/cargo" test --test vpn_docker native_vpn_routes_keep_commands_on_host_and_share_gateways -- --ignored --nocapture --exact'
```

Expected: actual MCP transport saves `vpnRoutes`, starts native fixture processes through two isolated VPN profiles, reads logs, stops a service, fails closed on gateway loss, and prints `fixtureCleanupComplete`.

- [ ] **Step 4: Start only the isolated desktop app**

```bash
rtk pnpm test-app
```

Expected: product `Service Manager Test`, identifier `dev.servicemanager.desktop.test`, isolated data directory, Vite port 1420, and no change to production MCP port 47831.

- [ ] **Step 5: Create a disposable fixture through the Test app**

Use a new directory under `/private/tmp`, add one command service that prints bounded markers and remains alive, then verify Run, Ready/Running, Restart, Stop, search, filters, follow, copy, and clear. Do not open or modify a registered production folder.

- [ ] **Step 6: Verify group and log workspace behavior**

Create one disposable group, collapse/expand it, move the fixture in/out using pointer and non-drag controls, open multiple fixture log tabs, reorder them, move them between panes, add more than two panes, close tabs/panes, and confirm restored layout after reopening the view.

- [ ] **Step 7: Verify settings surfaces without secrets**

Open Service Settings, History, Diagnostics, Docker VPN, and AI Connections. Save a synthetic VPN route against a synthetic profile only when the existing fixture provides one. Enable isolated MCP 47832 with a disposable client and verify list/status/log reads. Never enter organization credentials, PINs, tokens, or `.ovpn` contents into test evidence.

- [ ] **Step 8: Verify the visual/accessibility matrix**

Check 900×600, 1200×820, and a large window in light/dark/system themes and minimum/default/maximum opacity. Complete the critical checks:

- no horizontal page scroll;
- full keyboard access and visible focus;
- selected service/view/tab state is announced;
- status remains understandable without color;
- reduced motion removes nonessential transitions;
- logs, fields, alerts, and command beam remain readable;
- no sticky UI obscures focused controls.

- [ ] **Step 9: Write bounded evidence**

Create `.agents/evidence/TASK-0025/native-ops-console-smoke.md` containing the branch/commit, exact commands and exit results, Test app identifier/data/MCP port, disposable fixture path, states observed, window/theme/opacity matrix, and any remaining risk. Do not include raw logs, tokens, credentials, private paths outside the disposable fixture, or screenshots containing organization data.

- [ ] **Step 10: Stop the fixture and Test app**

Use the Test app Stop control, confirm the fixture is terminal and its declared port is closed, revoke the disposable MCP client, disable isolated MCP, and close the Test app. Confirm production listener 47831 and production service run IDs were not changed.

- [ ] **Step 11: Commit verification evidence**

```bash
rtk git add .agents/evidence/TASK-0025/native-ops-console-smoke.md
rtk git commit -m "test(ui): verify native ops console"
```

Do not run `tauri build`, create a DMG, install an app, replace `/Applications/Service Manager.app`, push, publish, or deploy.
