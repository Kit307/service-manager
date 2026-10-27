# Native services through shared Docker VPNs

Service Manager keeps Go, Node and other shell commands native on macOS. Docker runs only the owned Gluetun OpenVPN gateways. A command service opts in per TCP destination by opening a loopback relay:

```text
native service → 127.0.0.1:localPort → selected VPN → remoteHost:remotePort
```

Traffic not sent to a declared relay keeps using the normal Mac network. There is no automatic full-subnet interception or route between two VPNs.

## Setup

1. Open **Docker VPN** and explicitly download `qmcgaw/gluetun:v3.41.0`.
2. Import an inline-certificate `.ovpn` profile. Credentials and private-key passphrases stay in the app’s private data directory.
3. Configure health/DNS values required by that VPN and connect it. Dynamic PIN prompts are answered only in the desktop app.
4. Open a command service’s **Network** section and add one or more routes: VPN profile, destination hostname, destination TCP port and an unused local loopback port.
5. Point the application setting for that dependency at `127.0.0.1:<localPort>`. The saved run command, cwd and env file do not move into Docker.

Example service configuration:

```json
{
  "id": "api",
  "name": "API",
  "cwd": ".",
  "type": "command",
  "command": "go run .",
  "envFile": ".env",
  "vpnRoutes": [
    {
      "profileId": "DISCOVERED_PROFILE_ID",
      "remoteHost": "database.internal.example",
      "remotePort": 5432,
      "localPort": 15432
    }
  ]
}
```

For this example the application must use `127.0.0.1:15432` instead of the private database endpoint. HTTP clients that must preserve the original hostname or TLS SNI need application-level proxy support; a raw loopback TCP endpoint cannot transparently replace every HTTPS URL.

## Lifecycle and isolation

- Several services using one profile share its single gateway.
- One service can hold routes through several profiles at the same time.
- Relay listeners bind only to `127.0.0.1`.
- Run waits for every selected gateway and every local listener before starting the native command.
- Stop closes the relays and releases the profile leases.
- If the exact owned gateway stops or its identity or ownership changes, the native process and relays stop. There is no direct-network fallback.
- A transient PIN or reauthentication state may keep an already running service alive while the same owned firewall gateway remains present; new starts still require Ready.

## AI/MCP

Use `list_vpn_profiles {}` to discover profile IDs and readiness. `upsert_service` accepts `vpnRoutes` through the normal revision-checked service definition. VPN import, connect/disconnect, credential entry, PIN entry and image downloads remain desktop-only actions. Never place `.ovpn` contents, credentials, passphrases or PINs in project config or chat.

## Limits

- TCP only; no UDP.
- No transparent process routing, macOS Network Extension, full private subnet bridge or VPN-to-VPN bridge.
- Each local relay port must be unique and unused.
- Private DNS resolution occurs inside the selected gateway’s proxy.
- Real organization VPN policy, DNS and MFA behavior still need a local profile smoke test after synthetic verification.
- Legacy profile-level published ports and Linux worker images remain readable for backward compatibility, but new service configuration should use `vpnRoutes`.
