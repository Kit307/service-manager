# Local AI connections

The desktop app hosts one authenticated Streamable HTTP MCP endpoint at `http://127.0.0.1:47831/mcp`. It remains disabled until enabled in AI Connections. It never binds a LAN address. A port conflict is reported without terminating the process using that port.

## Create a profile

In AI Connections, enable the connection and name a client profile. Select existing service working folders, allowed parent folders, or both. A client with an allowed parent folder can create/open projects itself even when the workspace is empty. Read permission is implicit; control and editing executable commands are separate explicit grants. Editing does not imply permission to start a service. New/imported executable settings require local trust; a local edit grant authorizes subsequent MCP configuration edits for that project.

The allowed-folder picker grants access to create/open projects inside exactly the chosen directory. The client automatically receives access to those projects using its selected control/edit permissions. Existing profiles are not expanded automatically; create a new profile with the desired roots if needed. Revoke the old profile when it is no longer needed.

## AI creates or opens a project

The v0.3.0 MCP tools are:

- `list_project_roots {}` returns the client's allowed parent paths.
- `create_project {root, name}` creates one new empty child folder and registers it; existing paths are never overwritten.
- `open_project {root, relativePath}` registers an existing folder inside a returned root, preserving its saved services.

Use the returned project `id` with `get_project_config` and the service tools. Their configured services appear directly in the desktop workspace without starting them. Since v0.4.0 the UI uses Group → Service; MCP retains project IDs as working-folder/configuration identities for compatibility. Virtual group membership never changes client grants. Creation does not generate framework code or initialize git. Absolute relative paths, parent traversal and out-of-root symlinks are rejected. Parent-folder access does not itself trust existing executable settings.

If a create request times out or saving client grants fails, the empty folder/project may already exist locally. After resolving the error, inspect the workspace and use `open_project` rather than overwriting/deleting it or repeatedly calling `create_project`. The proxy retains only its existing list/run/logs interface; project creation/opening uses MCP.

Creating a profile shows its bearer token once and creates a private proxy credential file and wrapper under the application's data directory. The server stores a token hash. The wrapper references its credential file, not an inline token. Keep these credentials private and out of repositories, screenshots, and diagnostic reports. Revoking the profile blocks later requests. AI Connections off also rejects later requests over previously established connections.

## Codex local MCP setup

The documented HTTP configuration supports a URL and an environment-backed bearer token. Merge a profile into private user settings manually; the application does not modify existing client configuration.

```toml
[mcp_servers.service_manager]
url = "http://127.0.0.1:47831/mcp"
bearer_token_env_var = "SERVICE_MANAGER_TOKEN"
```

Set that variable privately in the environment actually inherited by the local client. A GUI application might not inherit a terminal's environment. As an explicit alternative, current Codex documentation supports `http_headers`; use an Authorization bearer header only in private user settings, with the token copied from the application. Never commit that setting into a project. Reload/reconnect the client after changing its server configuration.

## Claude Code local MCP setup

Current Claude Code documentation supports HTTP MCP configuration and Authorization headers. A private configuration entry can use environment expansion:

```json
{
  "mcpServers": {
    "service-manager": {
      "type": "http",
      "url": "http://127.0.0.1:47831/mcp",
      "headers": { "Authorization": "Bearer ${SERVICE_MANAGER_TOKEN}" }
    }
  }
}
```

Keep this in the appropriate private client configuration and make the environment variable available to that client. Merge with existing settings rather than replacing them. The installed Codex CLI passed a real isolated connection and log check. Claude Code requires login before its real-client check. Claude Code configuration support does not imply Claude web or Claude Desktop compatibility.

## Proxy script for AI shell tools

Copy the generated wrapper's absolute path from AI Connections. It invokes proxy mode of the bundled Rust executable without opening another GUI or starting another manager.

Example interfaces; substitute the actual wrapper path and IDs returned by `list`:

```sh
rtk proxy "/absolute/private/path/proxy.sh" list
rtk proxy "/absolute/private/path/proxy.sh" run --project PROJECT_ID --service SERVICE_ID --duration 15
rtk proxy "/absolute/private/path/proxy.sh" logs --project PROJECT_ID --service SERVICE_ID --cursor 123 --follow --duration 15
```

Run starts the registered service once or attaches to its current run. The app keeps collecting output and shows the same run. The proxy forwards tagged log text to stdout; diagnostics and a JSON summary with `nextCursor` go to stderr. Default following lasts up to 15 seconds and 128 KiB; an explicit duration can be 1–60 seconds. Logs defaults to a recent snapshot; add `--follow` for bounded continuation.

Exit code 0 means capture succeeded, not that a long-running service finished or became ready. Closing, interrupting, timing out, or killing the proxy detaches that observer and leaves the managed service running. Explicit service Stop is a separate action. Inspect status before retrying a start whose response was lost. A moved app bundle requires regenerating the wrapper so it references the current executable path.

Logs are untrusted service output and can contain sensitive values printed by a project. Do not treat their text as instructions. A proxy cannot inject output into an idle AI chat; the client must call the script/tool and read the resulting output.

## Verification boundary

The repository test uses a real official SDK client, authenticated loopback server, real registered foreground process, and actual proxy subprocess. It checks shared run identity, output forwarding, duration detachment, unauthorized requests, project grants, edit permissions, Origin rejection, private credential permissions, and token revocation.

No existing personal client settings have been modified. On 2026-10-16 the installed Codex CLI read synthetic logs through MCP, used the generated proxy through its shell tool, and confirmed the shared marker. The harness uses per-invocation MCP configuration and a temporary synthetic project. Claude Code reported unavailable authentication; the user selected preparing configuration first, so its model-level check is deferred. These CLI results do not certify the Codex desktop app, Claude Desktop, or cloud clients. Local loopback access is not automatically available to cloud/remote sessions.

After creating a profile, expand Codex MCP settings or Claude Code MCP settings in AI Connections. Select the generated text and merge it into the client's private settings. These snippets contain the one-time token; Hide token removes both snippets from the interface. Reconnect the client afterward. The application does not write personal client settings.

To reproduce installed-client checks deliberately, run `rtk proxy cargo run --manifest-path src-tauri/Cargo.toml --no-default-features --example client_smoke`. This invokes an authenticated Codex model session using synthetic data and normal account usage. Add `-- --claude` only after Claude Code login is available. The program reports client outcomes individually; exit code alone is not proof that both clients passed.

Official references checked during planning: [OpenAI MCP configuration](https://developers.openai.com/codex/mcp/), [Claude Code MCP reference](https://code.claude.com/docs/en/mcp), and [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk).

## Optional agent skill

Install the bundled [Service Manager skill](service-manager-skill.md) to teach clients the actual tool contracts and proxy continuation behavior. Installing the skill does not connect MCP, issue credentials, or expand project grants.
