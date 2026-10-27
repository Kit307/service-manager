# Native Ops Console Redesign

## Objective

Redesign the Service Manager desktop interface as a focused macOS operations console while preserving every existing service, group, log, VPN, AI connection, and lifecycle behavior.

The redesign must improve contrast, information hierarchy, density, and consistency without changing backend APIs, persisted project configuration, service commands, process ownership, MCP contracts, or VPN routing.

## Selected Direction

The approved direction is **Native Ops Console**.

It uses restrained graphite and steel surfaces over the existing native Liquid Glass window. Operational colors appear only when they communicate state. Logs remain the dominant workspace instead of being reduced by decorative cards or summary metrics.

The defining visual element is a continuous command-and-status beam beneath the service header. It gives the selected process one stable operational center: command, path, PID, trust, health, and lifecycle actions remain visually connected without becoming one crowded card.

## Product Principles

1. Preserve behavior before changing appearance.
2. Give the selected service and its output the most space.
3. Use color for status and risk, not decoration.
4. Keep frequent actions stable across every service state.
5. Prefer native controls, existing SVG icons, and platform fonts over new dependencies.
6. Keep glass readable: important content receives sufficient opacity, edge contrast, and separation.

## Visual System

### Color

- Canvas: deep neutral graphite in dark mode and cool mist gray in light mode.
- Glass surfaces: neutral translucent layers with visible rims and restrained shadows.
- Primary text: high-contrast neutral.
- Secondary text: must retain at least 4.5:1 contrast at normal text sizes.
- Ready/running: green.
- Starting/stopping/warning: amber.
- Failed/destructive: red.
- Focus: a consistent two-pixel high-contrast ring with an offset.

Opacity remains user-adjustable, but lowering the window background opacity must not reduce the effective opacity of log output, form fields, alerts, or other reading-critical surfaces below a readable threshold.

### Typography

- UI: macOS system sans-serif stack.
- Commands, paths, PIDs, ports, and logs: SF Mono-compatible system monospace stack.
- No web font download or font dependency.
- Normal body content remains at least 13px in the dense desktop layout; helper text must remain legible and high contrast.

### Shape and Motion

- Compact radii distinguish structure: 8–12px panels and pill-shaped segmented controls.
- One consistent outline icon family using the existing `Icon` component.
- Hover, selection, and pane transitions use 120–180ms motion.
- `prefers-reduced-motion` removes nonessential transitions.
- Pressed states must not move surrounding layout.

## Application Layout

```text
┌ Sidebar ┬ Context bar ─────────────────────────┐
│ Groups  │ Service title   Status   Run actions │
│ Service │ Command / path / PID                 │
│ Status  ├ Output | Settings | History | Diag ─┤
│         │                                      │
│ VPN     │       Log / selected content         │
│ Logs    │                                      │
│ AI      │                                      │
└─────────┴──────────────────────────────────────┘
```

The window remains resizable with the existing 900×600 minimum. At narrower widths, metadata wraps or collapses before primary actions move. No view may introduce horizontal page scrolling at the minimum size.

## Component Design

### Application Shell

`App.tsx` retains navigation, selection, lifecycle, log subscription, appearance, and opacity state. The redesign changes its structural markup only where required for clearer regions and accessible labels.

The shell contains:

- a compact service sidebar;
- a context bar for group/service location and workspace state;
- a selected-service operation header;
- stable view tabs;
- one primary content region.

### Service Sidebar

`ServiceSidebar` retains group collapse, selection, drag/drop, ungrouping, and log-workspace drop behavior.

- Group names lead each section; counts are secondary.
- Service rows expose name and status without relying on color alone.
- Selected, hovered, focused, and dragged states remain distinct.
- Drag operations keep existing button/menu alternatives for pointer-independent use.
- Docker VPN, Log workspace, AI Connections, appearance, opacity, and running count remain in the utility area.

### Group Overview

`GroupOverview` becomes an operational list rather than a bento or metric-card dashboard.

- Run-all controls remain visible and stable.
- Dependency order, current state, and action result remain readable in one scan.
- Empty groups provide a direct next action.
- No decorative aggregate cards consume log or service space.

### Selected Service

The service header keeps the name, kind, path, PID, state, trust, health, command, and lifecycle controls.

- Run is the primary action only for stopped or failed services.
- Stop is destructive but uses a restrained outline/tint instead of a large red surface.
- Restart remains secondary.
- Starting and stopping show non-flashing progress text.
- The command-and-status beam opens Settings through the existing control and never edits the command directly.

The view rail keeps Output, Settings, History, Diagnostics, and Open in log workspace. The active view uses `aria-pressed` or equivalent selected semantics.

### Logs

`LogPanel` and `LogWorkspace` share one console language:

- stable dark reading surface in both themes;
- monospace output with aligned timestamp and stream metadata;
- filter, search, follow, copy, and clear controls remain available;
- severity uses label/icon plus color;
- truncated/gap information remains explicit;
- log text remains plain and untrusted.

The workspace retains unlimited panes, tab reorder, cross-pane moves, drag-to-new-pane, closing, and persisted layout. Existing move-left, move-right, and move-to-pane controls remain the keyboard/single-pointer alternatives to dragging.

### Settings, VPN, and AI Connections

`ServiceEditor`, `VpnSettings`, `GroupEditor`, and `AiConnections` use the same form grammar:

- visible section titles;
- persistent labels;
- helper text below the relevant field;
- inline field errors;
- one stable action row;
- destructive actions visually separated from save actions.

No VPN credential, imported configuration, token, PIN, or private log content is added to the public UI or test fixtures.

## Interaction and Feedback

- Every clickable control uses a native interactive element.
- Icon-only controls retain descriptive accessible names and selected/expanded state where applicable.
- Keyboard focus order follows visual order.
- Sticky regions cannot obscure focused elements.
- Disabled controls are semantically disabled and visually distinct.
- Important application errors appear in a persistent top banner with a concrete recovery direction.
- Field failures stay beside the field and retain the user's input.
- Starting, stopping, refreshing, connecting, and copying provide visible progress or completion feedback.

## Technical Boundaries

The redesign may change React component markup, CSS modules/files, semantic labels, and narrowly required UI tests.

It must not change:

- Rust service lifecycle behavior;
- process execution or ownership;
- project/config schema;
- MCP tools, token policy, or ports;
- VPN gateway, route, credential, or MFA behavior;
- log retention or redaction rules;
- current commands, env files, paths, or registered services;
- installed production application or its data.

No new frontend dependency is planned. Existing components, icon paths, CSS capabilities, Tauri native glass, and platform fonts are sufficient.

## Verification Strategy

### Automated

- Run all React tests and add focused behavior tests only where markup or accessibility semantics change.
- Run TypeScript, ESLint, and frontend production compilation.
- Run the existing Rust suite to prove the presentation work did not alter desktop contracts.
- Run the synthetic MCP/VPN relay proof when affected files or fixtures overlap.

### Isolated Desktop Smoke Test

Use `Service Manager Test`, identifier `dev.servicemanager.desktop.test`, separate application data, and MCP port 47832.

Create only disposable fixture projects/services and verify:

1. open a folder and create a service;
2. create/collapse/edit a group;
3. run, stop, and restart a native service;
4. receive, search, filter, follow, copy, and clear logs;
5. open several log tabs, reorder them, move them between panes, and create additional panes;
6. use pointer and keyboard alternatives for drag operations;
7. open Settings, History, Diagnostics, Docker VPN, and AI Connections;
8. save and display a VPN route without using real credentials;
9. connect to the isolated MCP endpoint and read service state/logs;
10. confirm Stop removes the fixture process and leaves production services unchanged.

### Visual and Accessibility Matrix

- Window sizes: 900×600, 1200×820, and a large desktop window.
- Themes: light, dark, and system.
- Background opacity: minimum, default, and maximum.
- Input: pointer and keyboard-only.
- Motion: normal and reduced-motion.
- Check text contrast, focus visibility, overflow, clipped controls, tab order, selected state, status comprehension without color, and log readability.

## Acceptance Criteria

- The visible result matches the approved Native Ops Console direction.
- All existing user-visible behaviors remain available and produce the same outcomes.
- Selected service, lifecycle state, and primary action are understandable in one scan.
- Logs remain the largest working area on the service screen.
- No horizontal page scroll appears at 900×600.
- Light/dark/system themes and opacity controls preserve readable content.
- Drag operations have accessible alternatives.
- Automated verification and isolated real-app smoke tests pass.
- Production application and current services are not installed, restarted, stopped, or modified.

## Delivery Boundary

Implementation stops after verified source and isolated Test app evidence. Production bundle creation, installation, replacement, restart, publishing, or deployment requires a separate explicit request.
