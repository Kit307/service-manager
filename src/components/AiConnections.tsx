import { useState } from "react";
import { api, confirmAction } from "../lib/api";
import type { Workspace } from "../lib/types";
import { Icon } from "./Icon";

export function AiConnections({
  workspace,
  onRefresh,
  onError,
}: {
  workspace: Workspace;
  onRefresh: () => Promise<unknown>;
  onError: (error: unknown) => void;
}) {
  const [name, setName] = useState("Codex");
  const [busy, setBusy] = useState(false);
  const [credential, setCredential] = useState<{
    token: string;
    id: string;
    proxyPath: string;
  }>();
  async function perform(action: () => Promise<unknown>) {
    setBusy(true);
    try {
      await action();
      await onRefresh();
    } catch (e) {
      onError(e);
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="ai-page">
      <div className="ai-heading">
        <span className="welcome-icon small">
          <Icon name="ai" size={24} />
        </span>
        <h1>Your AI. The same workspace.</h1>
        <p>
          Let Codex and Claude work with the services you see here.
          <br />
          One manager, one run, the same output.
        </p>
      </div>
      <section className="settings-section">
        <div className="settings-row">
          <div>
            <h2>Local MCP connection</h2>
            <p>
              Available only on this machine. Every request needs a client
              token.
            </p>
          </div>
          <button
            className={workspace.ai.enabled ? "secondary" : "primary"}
            disabled={busy}
            onClick={() => {
              void perform(() => api.setAi(!workspace.ai.enabled));
            }}
          >
            {workspace.ai.enabled ? "Disable connection" : "Enable connection"}
          </button>
        </div>
        <div className="endpoint">
          <span
            className={`connection-dot ${workspace.ai.enabled ? "connected" : ""}`}
          />
          <code>{workspace.ai.endpoint}</code>
          <span>{workspace.ai.enabled ? "Listening locally" : "Disabled"}</span>
        </div>
        {workspace.ai.error && (
          <p className="runtime-error">{workspace.ai.error}</p>
        )}
      </section>
      <section className="settings-section">
        <h2>Connected client profiles</h2>
        <p>
          Every valid token can create or open projects in any folder accessible
          to this app, read logs, run, stop, restart and edit all services.
          Existing tokens have the same full access.
        </p>
        {!workspace.ai.clients.length && (
          <p className="empty-inline">
            No client profiles yet. Create one for each AI client.
          </p>
        )}
        {workspace.ai.clients.map((client) => (
          <div className="client-row" key={client.id}>
            <Icon name="ai" />
            <div>
              <strong>{client.name}</strong>
              <span>
                Full access · Create/open projects + control + edit commands
              </span>
            </div>
            <button
              className="tiny danger-text"
              disabled={busy}
              onClick={async () => {
                if (
                  await confirmAction(
                    `Revoke ${client.name}? Its token and proxy access will stop working.`,
                    "Revoke client?",
                  )
                )
                  void perform(() => api.revokeClient(client.id));
              }}
            >
              Revoke
            </button>
          </div>
        ))}
      </section>
      <form
        className="settings-section"
        onSubmit={(e) => {
          e.preventDefault();
          void perform(async () =>
            setCredential(
              await api.createClient(name, [], true, true),
            ),
          );
        }}
      >
        <h2>Create a client profile</h2>
        <section className="form-section" aria-labelledby="ai-connection-settings-heading">
        <h2 id="ai-connection-settings-heading">Connection settings</h2>
        <label>
          Client name
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            maxLength={80}
          />
        </label>
        <p className="warning-banner">
          Full access: this token can create/open projects and edit/execute
          commands anywhere your user account allows. No additional folder grants
          are needed. macOS privacy permissions still apply. Keep this token private.
        </p>
        </section>
        <div className="form-actions" role="group" aria-label="Form actions">
        <button
          className="primary"
          disabled={busy || !workspace.ai.enabled || !name.trim()}
        >
          <Icon name="plus" />
          Create profile & proxy
        </button>
        </div>
      </form>
      {credential && (
        <section className="settings-section credential">
          <div className="settings-row">
            <h2>Save your connection details</h2>
            <button className="tiny" onClick={() => setCredential(undefined)}>
              Hide token
            </button>
          </div>
          <p>
            This token is shown once. Keep it in private client settings, never
            in project files.
          </p>
          <label>
            Bearer token
            <input
              className="mono"
              readOnly
              value={credential.token}
              onFocus={(e) => e.target.select()}
            />
          </label>
          <label>
            Proxy script
            <input
              className="mono"
              readOnly
              value={credential.proxyPath}
              onFocus={(e) => e.target.select()}
            />
          </label>
          <details>
            <summary>Codex MCP settings</summary>
            <p>
              Paste into your private Codex config.toml, then reconnect. This
              contains your token.
            </p>
            <textarea
              aria-label="Codex MCP configuration"
              className="mono"
              readOnly
              rows={5}
              value={`[mcp_servers.service_manager]\nurl = ${JSON.stringify(workspace.ai.endpoint)}\nhttp_headers = { Authorization = ${JSON.stringify(`Bearer ${credential.token}`)} }`}
              onFocus={(e) => e.target.select()}
            />
          </details>
          <details>
            <summary>Claude Code MCP settings</summary>
            <p>
              Merge into your private MCP settings, then reconnect. Claude Code
              must be logged in.
            </p>
            <textarea
              aria-label="Claude Code MCP configuration"
              className="mono"
              readOnly
              rows={12}
              value={JSON.stringify(
                {
                  mcpServers: {
                    service_manager: {
                      type: "http",
                      url: workspace.ai.endpoint,
                      headers: { Authorization: `Bearer ${credential.token}` },
                    },
                  },
                },
                null,
                2,
              )}
              onFocus={(e) => e.target.select()}
            />
          </details>
          <p>
            Use the proxy through your AI's shell tool to receive service logs
            on stdout. Closing the proxy leaves the service running.
          </p>
        </section>
      )}
    </div>
  );
}
