# Service Manager 0.5 MCP contract

Use the actual tools exposed by the client's configured Service Manager server. The server is authenticated Streamable HTTP at `http://127.0.0.1:47831/mcp` by default. Do not handcraft protocol requests or put a token in a URL; use the configured MCP client or generated proxy.

| Tool | Arguments | Result |
| --- | --- | --- |
| `list_projects` | `{}` | `{projects: [{id, name}]}` for all registered working folders, including later additions |
| `list_project_roots` | `{}` | `{roots: [{path}]}` for this client's parent-folder grants |
| `create_project` | `{root, name}` | Project `{id, name, path, config, revision, trusted, runtime}`; creates one new empty child folder |
| `open_project` | `{root, relativePath}` | Same project shape; registers an existing folder inside the root |
| `list_services`, `get_project_config` | `{projectId}` | `{config, revision, trusted, runtime}` |
| `get_service_status` | `{projectId, serviceId}` | Current snapshot |
| `start_service`, `stop_service`, `restart_service` | `{projectId, serviceId}` | Snapshot; observe completion through status |
| `read_logs` | `{projectId, serviceId, cursor?}` | `{records, nextCursor, truncated}` |
| `upsert_service` | `{projectId, definition, expectedRevision}` | `{config, revision, trusted}`; does not start |

Snapshots include `state`, `health`, `activity`, and optional `runId`, `pid`, `exitCode`, `error`. States: stopped, starting, running, ready, stopping, failed. Log records include seq, runId, timestamp, stream, text, truncated. MCP read_logs has no server-side stream/run filters in 0.2; filter returned records locally. Proxy logs supports `--run`.

## Create or open a project

In AI Connections, the user selects allowed parent folders for a client. Creating a token requires only a name. Every valid token, including existing tokens after upgrading to v0.5.0, grants full read/control/edit access to every registered working folder. Parent-folder grants are optional and remain necessary only for filesystem creation/opening. Existing profiles receive no new filesystem roots automatically.

1. Call `list_project_roots` and use an exact returned `path` as `root`.
2. For a new project, call `create_project` with `name` set to a single new child folder name. It will not overwrite an existing path, scaffold files, initialize git, or run commands.
3. For existing code, call `open_project` with a relative folder path under that root. `relativePath: "."` opens the root itself. Absolute paths, parent traversal and symlinks resolving outside the root are rejected. If the granted root was replaced with an outside symlink, select it again locally.
4. Use the returned `id` with the service tools. Registered services become available to every valid token. Opening does not run them. Successful MCP upserts record trust; MCP start/restart authorizes the current registered command before the shared manager revalidates it.

If folder grants are empty, the user must select an allowed folder locally. There is no automatic machine-wide filesystem grant. Proxy commands remain `list`, `run`, and `logs`; creation/opening requires MCP.

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
  "healthCheck": {"url": "http://127.0.0.1:8080/health"}
}
```

`envFile` and `healthCheck` are optional. `cwd`, `envFile`, and Compose `file` are relative to the project root and must exist within it. Command and Compose are mutually exclusive variants: Compose uses `type: compose` plus `file: compose.yaml`, without `command`. Project-file schema is `{schemaVersion: 1, services: [...]}`; upsert takes one definition, not the whole config.

A valid v0.5.0 token authorizes executable changes. Accepted MCP upserts and MCP start/restart record trust for the current definition, then the manager revalidates configuration. No second local trust dialog is required for these MCP operations. External replacement after trust can still fail revalidation. Never edit the private trust store yourself.

On `CONFIG_CONFLICT`, fetch the latest config, compare the intended edit and preserve concurrent work; do not blindly resend the stale revision. On `OPERATION_IN_PROGRESS`, query status before another mutation. On Docker/stop errors, retain the actual active/stopping state and error; do not kill by port/name or use down/prune. Compose supports local owned long-running services; unsupported shared/scaled configurations are rejected by the manager.
