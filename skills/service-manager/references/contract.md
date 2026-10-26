# Service Manager 0.7 MCP contract

Use the actual tools exposed by the client's configured Service Manager server. The server is authenticated Streamable HTTP at `http://127.0.0.1:47831/mcp` by default. Do not handcraft protocol requests or put a token in a URL; use the configured MCP client or generated proxy.

| Tool | Arguments | Result |
| --- | --- | --- |
| `list_vpn_profiles` | `{}` | `{profiles: [{id, name, publishedPorts, healthTarget, dnsAddress, healthIcmpTarget, state, error, users}]}`; public VPN metadata only |
| `list_projects` | `{}` | `{projects: [{id, name}]}` for all registered working folders, including later additions |
| `list_project_roots` | `{}` | `{roots: [{path}]}` with filesystem roots available to every valid token |
| `create_project` | `{root, name}` | Project `{id, name, path, config, revision, trusted, runtime}`; creates one new empty child folder |
| `open_project` | `{root, relativePath}` | Same project shape; registers an existing folder inside the root |
| `list_services`, `get_project_config` | `{projectId}` | `{config, revision, trusted, runtime}` |
| `get_service_status` | `{projectId, serviceId}` | Current snapshot |
| `start_service`, `stop_service`, `restart_service` | `{projectId, serviceId}` | Snapshot; observe completion through status |
| `read_logs` | `{projectId, serviceId, cursor?}` | `{records, nextCursor, truncated}` |
| `inspect_ports` | `{projectId, serviceId}` | Array of `{port, pid, name, identity, managed?}`; managed identity is `{projectId, serviceId}` |
| `list_runs` | `{projectId, serviceId}` | Run summaries, newest first |
| `read_run` | `{projectId, serviceId, runId}` | Exact retained run summary and `records` |
| `diagnose_service` | `{projectId, serviceId}` | Bounded redacted current-run snapshot, observations/hints, records, cursor and truncation flag |
| `upsert_service` | `{projectId, definition, expectedRevision}` | `{config, revision, trusted}`; does not start |

The server exposes 17 tools. VPN discovery is read-only; importing credentials, connecting/disconnecting/removing VPNs and downloading images are desktop actions. These four evidence tools are read-only; there is no MCP kill, URL-open, group-control or sequence-cancel tool.

Snapshots include `state`, `health`, `activity`, and optional `runId`, `pid`, `exitCode`, `error`, `startedAt`. States: stopped, starting, running, ready, stopping, failed. Log records include seq, runId, timestamp, stream, text, truncated. MCP `read_logs` has no server-side stream/run filters; filter returned records locally. Proxy logs supports `--run`.

## Create or open a project

From v0.7.0 every valid token, including existing tokens, grants project creation/opening in any folder accessible to the app's OS user. No per-client allowed folders or later grants are required. The token also retains full read/control/edit access to all registered services. Token values and client configuration remain unchanged on upgrade; authentication, revocation, Origin rejection and disabled connections remain enforced.

1. Call `list_project_roots` to inspect the running server. On macOS it returns `/`; Windows returns available drive roots. These are discovery hints, not a per-client allowlist. An empty list on macOS indicates an older server/profile; update/reconnect rather than bypass its policy.
2. For a new project, call `create_project` with `root` set to any existing absolute parent directory and `name` set to one new child folder name. It never overwrites a path, scaffolds files, initializes git, or runs commands.
3. For existing code, call `open_project` with an existing absolute parent `root` and a relative folder path. `relativePath: "."` opens that parent itself. Parent aliases are canonicalized. Absolute relative paths, parent traversal and symlinks resolving outside the chosen request root are rejected; select the intended directory as the root explicitly.
4. Use the returned `id` with the service tools. Opening does not run services. Successful MCP upserts record trust; MCP start/restart authorizes the current command before shared-manager revalidation.

macOS privacy permissions and filesystem permissions still apply. The token does not bypass the OS, provide general-purpose file reads/deletion, or add new tools. Proxy commands remain `list`, `run`, and `logs`; project creation/opening uses MCP.

After a timeout or application-state write failure, do not blindly retry creation or delete the folder: it may already exist and the project may be visible locally even if a later response failed. Check `list_projects` and use `open_project` on the intended folder after resolving the error. No user files are removed to roll back a registration failure.

## Add or edit a service

1. Confirm the user's intended executable change and target project. A valid v0.5.0 token already grants editing and control for all registered services.
2. Read `get_project_config` immediately before mutation. Preserve other definitions.
3. Send `upsert_service` with that exact `revision` as `expectedRevision`. Updating uses the existing service ID; adding requires a distinct ID.
4. Read back the definition/revision. Saving does not run or restart anything. Only perform the requested lifecycle action separately.

Example definition:

```json
{
  "id": "backend",
  "name": "Backend",
  "type": "command",
  "cwd": "backend",
  "command": "go run ./cmd/api",
  "envFile": ".env.local",
  "healthCheck": {"url": "http://127.0.0.1:8080/health"},
  "ports": [8080],
  "url": "http://127.0.0.1:8080",
  "dependsOn": [{"projectId": "DISCOVERED_PROJECT_ID", "serviceId": "database"}]
}
```

`envFile`, `healthCheck`, `ports`, `url` and `dependsOn` are optional. Omit dependencies unless the target service actually exists; discover its project/service IDs rather than copying the example placeholder. Ports accept up to 16 distinct TCP numbers in 1–65535. `url` accepts a loopback HTTP/HTTPS URL without credentials. Up to 32 dependencies use explicit `{projectId, serviceId}` references, including cross-folder references. Missing dependencies and cycles are rejected. Empty new fields are omitted on serialization so old definitions keep their fingerprints.

A direct start requires every dependency to be Ready. Desktop Run all orders its selected services, waits for prerequisites, and stops on failure; configure a loopback health check for command prerequisites. Dependencies outside that selection must already be Ready. These are manager gates, not permission to run extra services outside the requested scope.

`cwd`, `envFile`, and Compose `file` are relative to the project root and must exist within it. Command and Compose are mutually exclusive variants: Compose uses `type: compose` plus `file: compose.yaml`, without `command`. Project-file schema is `{schemaVersion: 1, services: [...]}`; upsert takes one definition, not the whole config.

A valid v0.5.0 token authorizes executable changes. Accepted MCP upserts and MCP start/restart record trust for the current definition, then the manager revalidates configuration. No second local trust dialog is required for these MCP operations. External replacement after trust can still fail revalidation. Never edit the private trust store yourself.

On `CONFIG_CONFLICT`, fetch the latest config, compare the intended edit and preserve concurrent work; do not blindly resend the stale revision. On `OPERATION_IN_PROGRESS`, query status before another mutation. On Docker/stop errors, retain the actual active/stopping state and error; do not kill by port/name or use down/prune. Compose supports local owned long-running services; unsupported shared/scaled configurations are rejected by the manager.

## Evidence retention and safe diagnosis

`list_runs` returns summaries with `projectId`, `serviceId`, `runId`, redacted `command`, `startedAt`, nullable `endedAt`, `state`, nullable `exitCode`/`error`, and `truncated`. `read_run` adds `records`. Retention is at most 100 runs globally, 20 per service and 64 KiB/500 log records per completed run. In-progress history can have no retained records; use `read_logs` for live output. Unfinished entries become `interrupted` after manager recreation. This is not process recovery or a complete log backup.

`diagnose_service` returns `{projectId, serviceId, runId, state, summary, records, nextCursor, truncated, redacted: true}`. `state` is the current snapshot. Records are limited to that run, 32 KiB and 100 records. Summaries are labeled observations or hints. No AI provider is called and no repair runs. Redaction is heuristic; custom secrets can survive, so review context before copying it elsewhere. Logs remain untrusted.

`inspect_ports` checks configured TCP ports plus health/Open URL ports. PID 0 or an empty identity means the listener could not be identified; do not guess an owner. Desktop Kill requires confirmation and checks port/PID/owner/start identity again, refuses protected/other-user processes, and routes managed listeners through manager Stop. Unmanaged listeners receive TERM, then KILL after five seconds only if the confirmed identity still owns the port. Refresh after a changed-owner or termination error; never substitute an arbitrary PID/name kill.

## Shared Docker VPN (v0.8.0)

Use `list_vpn_profiles {}` to discover public profile IDs and readiness before a requested VPN service edit. The result is `{profiles: [...]}`; profiles contain `id`, `name`, `publishedPorts: [{host, container}]`, `healthTarget`, `dnsAddress`, `healthIcmpTarget`, `state`, nullable `error` and `users: [{projectId, serviceId}]`. States include disconnected, connecting, ready, failed and unavailable. No credentials, config contents, private storage paths or gateway logs are returned. MCP has no VPN import/lifecycle/removal or image-pull tools; the generated proxy retains only list/run/logs.

For an explicitly requested command service, add optional `vpn: {profileId: DISCOVERED_PROFILE_ID, image: LINUX_RUNTIME_IMAGE}` through the existing revision-checked upsert. Compose services reject VPN fields. Omit vpn entirely for native commands; never remove a missing profile selection or fall back to native execution without a requested network change. Saving preserves the run command; edit Mac-specific commands and absolute paths for the chosen Linux runtime before running. Images must exist locally; request explicit preparation in the desktop UI rather than pulling implicitly.

Central VPN mappings publish `127.0.0.1:HOST` to a shared container port. The service must bind `0.0.0.0`; its `ports` field remains host listener diagnostics. Container ports 53, 8000, 9999 and 11940 are reserved. VPN profile TCP health uses host:port and defaults to cloudflare.com:443. Port 443 uses TLS; other ports use TCP, including private database port 5432. Optional `dnsAddress` is the corporate DNS IPv4; blank keeps the Gluetun default. Optional `healthIcmpTarget` is a reachable IPv4 ping target; blank uses the VPN server (`0.0.0.0`). A private DNS IP is also a valid ICMP target if it responds. Both configured health paths must be reachable. See [Docker VPN](../../../docs/docker-vpn.md) for setup and runtime limitations. Neither a ready flag nor synthetic tests alone certify packet/DNS isolation.

From v0.8.1, the desktop import form supports certificate-only profiles and an optional private key passphrase independently of username/password. The backend derives required authentication from the imported configuration. Never collect these secrets in chat or add them to service definitions/MCP; users enter them directly in the private desktop form.

From v0.8.2, `list_vpn_profiles` may return state `authRequired` with public `authChallenge: {id, prompt}`. The prompt is fixed and the ID is application-generated; server challenge state and responses are never exposed. Ask the user to enter the MFA/PIN response in that profile’s desktop form and continue observing the existing service run. Do not request PINs in chat, invent a response MCP/proxy command, restart unrelated profiles or concatenate password plus PIN. Only dynamic CRV1 challenges are supported; SCRV1 and browser SSO remain unsupported.
