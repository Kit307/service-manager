import { useState } from "react";
import { api } from "../lib/api";
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
  const [projects, setProjects] = useState<string[]>([]);
  const [control, setControl] = useState(true);
  const [edit, setEdit] = useState(false);
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
        <p>Choose exactly which projects and actions each client may access.</p>
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
                {client.projects.length} projects · Read
                {client.control ? " + control" : ""}
                {client.edit ? " + edit commands" : ""}
              </span>
            </div>
            <button
              className="tiny danger-text"
              disabled={busy}
              onClick={() => {
                if (
                  window.confirm(
                    `Revoke ${client.name}? Its token and proxy access will stop working.`,
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
              await api.createClient(name, projects, control, edit),
            ),
          );
        }}
      >
        <h2>Create a client profile</h2>
        <label>
          Client name
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            maxLength={80}
          />
        </label>
        <fieldset className="project-grants">
          <legend>Allowed projects</legend>
          {workspace.projects.length ? (
            workspace.projects.map((p) => (
              <label key={p.id}>
                <input
                  type="checkbox"
                  checked={projects.includes(p.id)}
                  onChange={(e) =>
                    setProjects(
                      e.target.checked
                        ? [...projects, p.id]
                        : projects.filter((id) => id !== p.id),
                    )
                  }
                />
                {p.name}
              </label>
            ))
          ) : (
            <p>Open a project folder first.</p>
          )}
        </fieldset>
        <div className="grant-options">
          <label>
            <input
              type="checkbox"
              checked={control}
              onChange={(e) => setControl(e.target.checked)}
            />
            Run, stop and restart trusted services
          </label>
          <label>
            <input
              type="checkbox"
              checked={edit}
              onChange={(e) => setEdit(e.target.checked)}
            />
            Add and edit executable commands
          </label>
        </div>
        {edit && (
          <p className="warning-banner">
            Edit permission lets this client change commands that run with your
            user permissions. Only grant this to a client you trust.
          </p>
        )}
        <button
          className="primary"
          disabled={busy || !workspace.ai.enabled || !projects.length}
        >
          <Icon name="plus" />
          Create profile & proxy
        </button>
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
          <p>
            Use the proxy through your AI's shell tool to receive service logs
            on stdout. Closing the proxy leaves the service running.
          </p>
        </section>
      )}
    </div>
  );
}
