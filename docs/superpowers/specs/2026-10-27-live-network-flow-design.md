# Live Network Flow Design

## Intent

Add one overview page that answers: which managed services can reach which destinations, which VPN path each route uses, and whether payload bytes are moving through that path now.

The visualization must be evidence-based. A route is never shown as live merely because a service is running, a VPN is connected, or a TCP tunnel remains open.

## Goals

- Show `This Mac → Service → VPN path → Destination` for configured native VPN routes.
- Keep every configured route visible as a thin neutral line.
- Show a directional outbound pulse only after the relay's transmitted payload-byte counter increases.
- Show a directional inbound pulse only after the relay's received payload-byte counter increases.
- Return to neutral 1.5 seconds after the latest increase in that direction.
- Show active tunnel count and cumulative relay-scoped RX/TX totals for the selected route.
- Show services without configured native relay routes as `Not observable`; never invent a direct destination.
- Stop polling and animation when the page is not mounted.

## Non-goals

- Packet capture, DNS inspection, UDP accounting, kernel-wide monitoring, or Network Extension installation.
- Per-packet timing or exact bandwidth charts.
- Observing native direct traffic, legacy Docker VPN traffic, or traffic outside Service Manager-owned native TCP relays.
- Identifying or verifying which process used a relay.
- Persisting traffic history across runs or app restarts.
- Exposing traffic telemetry through MCP in this version.

## Approaches

### Selected: pull monotonic counters from owned relays

Each native relay keeps atomic counters for payload bytes successfully written in each direction and the number of established CONNECT tunnels. The desktop polls one bounded workspace snapshot every second and derives pulses from positive counter deltas.

This is exact for traffic the relay owns, needs no dependency or privilege, coalesces short requests safely, and adds no high-frequency event queue.

### Rejected: stream one event per read/write

Events would make animation timing more immediate but introduce backpressure, subscriber lifecycle, high event volume, and extra synchronization without improving the one-second overview.

### Rejected: inspect macOS sockets or packet traffic

Socket inspection cannot prove byte movement, attribution can be wrong after PID reuse, and accurate packet/byte monitoring would need platform-specific privileges or a Network Extension. The page must not imply this coverage.

## Truth model

`TX` means payload written from a local client of a service-configured loopback relay toward the remote destination after the HTTP CONNECT handshake succeeds. `RX` means payload written from the remote side back to that local client. Any local client can use the relay, so these counters are not verified service-process traffic.

Handshake bytes are excluded. Failed CONNECT attempts never become active tunnels and never increment payload counters. Open but idle tunnels may have `activeConnections > 0` while both counters remain unchanged; they produce no pulse.

A route instance is identified by `projectId`, `serviceId`, `runId`, and `localPort`. A new run, missing sample, counter decrease, or request failure resets the baseline and produces no pulse.

## Backend shape

The relay owns one shared counter object per configured route. Successful writes update a monotonic `u64` direction counter; an RAII guard updates the established-tunnel count through normal completion, failure, and cancellation.

The manager holds a non-persistent registry for current native relay sessions. Registration occurs only after both relay preparation and native process spawn succeed. A run-scoped guard removes only its own run, so stale cleanup cannot remove replacement telemetry. Gateway ownership loss drops the relay session and unregisters telemetry before the process is signalled.

The desktop command returns only current records:

```text
NetworkTraffic {
  projectId, serviceId, runId, profileId,
  localPort, remoteHost, remotePort,
  txBytes, rxBytes, activeConnections
}
```

The response contains no credentials, VPN configuration contents, auth challenges, or logs. The frontend resolves the profile display name from the existing workspace VPN list.

## Frontend behavior

Add `Live Network` beside the existing sidebar utility views. The page contains:

- compact totals for observable routes, active tunnels, and currently pulsing directions;
- a four-column flow canvas: `This Mac`, `Services`, `Network path`, `Destinations`;
- one selectable route row per configured native VPN route;
- a separate `Not observable` section for services without configured native relay routes;
- a selected-route detail panel with textual state, profile, local port, destination, active tunnels, RX and TX totals.

The base path is always neutral. A TX increase adds one blue outward dot; an RX increase adds one violet return dot. Color is decorative: the selected detail and route row also show `Sending`, `Receiving`, `Sending and receiving`, `Idle`, `Stopping`, `Failed`, `Stopped`, `Unavailable`, or `Not observable` text. The page states the relay's local-client attribution boundary.

The initial sample never animates. Polling is immediate then self-schedules one second after each completed request, preventing overlap. Each direction expires independently after 1.5 seconds. Late results after unmount are ignored.

## Accessibility and layout

- Route rows are native buttons with complete accessible names and `aria-pressed` selection.
- SVG paths and dots are decorative and hidden from assistive technology.
- Reduced motion removes moving dots but retains live textual state and counters.
- Forced colors uses system colors and preserves selected/focus boundaries.
- The graph owns horizontal overflow at 900×600; the application page itself does not horizontally scroll.
- Long service, profile, and destination labels truncate visually but remain complete in accessible names and titles.

## Error and lifecycle behavior

- Poll errors keep configured routes visible, clear pulses, label telemetry `Unavailable`, and retry on the normal cadence.
- Stopped routes remain neutral using configuration data; counters are absent rather than zeroed history.
- While a run is active, its route definitions cannot be changed or removed through saves or external reloads. Unrelated edits remain available; route changes are accepted after stop.
- Stopping routes retain their current-run telemetry and pulses until the relay is removed; failed runs are labeled separately from stopped runs.
- Removing a selected route selects the next surviving route or clears the detail panel.
- Traffic counters and registry entries disappear when a run ends, start fails, is stopped, or loses its owned VPN gateway.

## Acceptance

- Known different payload sizes increment TX and RX before the tunnel closes.
- Idle established tunnels show no pulse.
- Failed CONNECT and handshake bytes do not affect totals.
- Initial, stale, reset, and changed-run samples do not create phantom pulses.
- Multiple projects, profiles, routes, and identical destination labels remain distinct.
- Direct and Docker traffic are explicitly unobservable.
- Frontend, Rust, lint, typecheck, and build checks pass; the existing synthetic native VPN proof validates actual transport and cleanup.

