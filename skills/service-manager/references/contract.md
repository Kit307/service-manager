# Service Manager 0.2 MCP contract

Use the actual tools exposed by the client's configured Service Manager server. The server is authenticated Streamable HTTP at `http://127.0.0.1:47831/mcp` by default. Do not handcraft protocol requests or put a token in a URL; use the configured MCP client or generated proxy.

| Tool | Arguments | Result |
| --- | --- | --- |
| `list_projects` | `{}` | `{projects: [{id, name}]}` for granted projects |
| `list_services`, `get_project_config` | `{projectId}` | `{config, revision, trusted, runtime}` |
| `get_service_status` | `{projectId, serviceId}` | Current snapshot |
| `start_service`, `stop_service`, `restart_service` | `{projectId, serviceId}` | Snapshot; observe completion through status |
| `read_logs` | `{projectId, serviceId, cursor?}` | `{records, nextCursor, truncated}` |
| `upsert_service` | `{projectId, definition, expectedRevision}` | `{config, revision, trusted}`; does not start |

Snapshots include `state`, `health`, `activity`, and optional `runId`, `pid`, `exitCode`, `error`. States: stopped, starting, running, ready, stopping, failed. Log records include seq, runId, timestamp, stream, text, truncated. MCP read_logs has no server-side stream/run filters in 0.2; filter returned records locally. Proxy logs supports `--run`.

## Add or edit a service

1. Confirm the user's intended executable change and target project. The client needs an explicit local edit grant; control is a separate grant.
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
  "healthCheck": {"url": "http://127.0.0.1:8080/health"}
}
```

`envFile` and `healthCheck` are optional. `cwd`, `envFile`, and Compose `file` are relative to the project root and must exist within it. Command and Compose are mutually exclusive variants: Compose uses `type: compose` plus `file: compose.yaml`, without `command`. Project-file schema is `{schemaVersion: 1, services: [...]}`; upsert takes one definition, not the whole config.

An edit grant authorizes executable changes and the implementation records trust for accepted MCP upserts. Explain that effect when discussing grants; do not claim every AI edit always requires a second local trust dialog. External file edits can invalidate trust. Never edit the private trust store yourself.

On `CONFIG_CONFLICT`, fetch the latest config, compare the intended edit and preserve concurrent work; do not blindly resend the stale revision. On `OPERATION_IN_PROGRESS`, query status before another mutation. On Docker/stop errors, retain the actual active/stopping state and error; do not kill by port/name or use down/prune. Compose supports local owned long-running services; unsupported shared/scaled configurations are rejected by the manager.
