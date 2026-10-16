import { useState } from "react";
import type { ServiceDefinition } from "../lib/types";
import { Icon } from "./Icon";

export function ServiceEditor({
  service,
  saving,
  onSave,
  onCancel,
  onRemove,
}: {
  service?: ServiceDefinition;
  saving: boolean;
  onSave: (service: ServiceDefinition) => void;
  onCancel: () => void;
  onRemove?: () => void;
}) {
  const [name, setName] = useState(service?.name ?? "");
  const [id, setId] = useState(service?.id ?? "");
  const [type, setType] = useState(service?.type ?? "command");
  const [cwd, setCwd] = useState(service?.cwd ?? ".");
  const [command, setCommand] = useState(
    service?.type === "command" ? service.command : "",
  );
  const [file, setFile] = useState(
    service?.type === "compose" ? service.file : "compose.yaml",
  );
  const [envFile, setEnvFile] = useState(service?.envFile ?? "");
  const [health, setHealth] = useState(service?.healthCheck?.url ?? "");
  return (
    <form
      className="service-form"
      onSubmit={(event) => {
        event.preventDefault();
        const common = {
          id,
          name: name.trim(),
          cwd: cwd.trim() || ".",
          ...(envFile.trim() ? { envFile: envFile.trim() } : {}),
          ...(health.trim() ? { healthCheck: { url: health.trim() } } : {}),
        };
        onSave(
          type === "command"
            ? { ...common, type, command: command.trim() }
            : { ...common, type, file: file.trim() },
        );
      }}
    >
      <div className="section-heading">
        <div>
          <h2>{service ? "Service settings" : "Add a service"}</h2>
          <p>A command, a working folder, and a place to watch it run.</p>
        </div>
      </div>
      <div className="form-row">
        <label>
          Service name
          <input
            autoFocus
            required
            value={name}
            placeholder="Frontend"
            onChange={(e) => {
              setName(e.target.value);
              if (!service)
                setId(
                  e.target.value
                    .toLowerCase()
                    .replace(/[^a-z0-9]+/g, "-")
                    .replace(/^-|-$/g, ""),
                );
            }}
          />
        </label>
        <label>
          Service ID
          <input
            required
            pattern="[a-z0-9][a-z0-9_-]*"
            readOnly={!!service}
            value={id}
            placeholder="frontend"
            onChange={(e) => setId(e.target.value)}
          />
        </label>
      </div>
      <fieldset className="type-options">
        <legend>Run with</legend>
        <label className={type === "command" ? "selected" : ""}>
          <input
            type="radio"
            name="type"
            value="command"
            checked={type === "command"}
            onChange={() => setType("command")}
          />
          <Icon name="terminal" />
          Shell command
        </label>
        <label className={type === "compose" ? "selected" : ""}>
          <input
            type="radio"
            name="type"
            value="compose"
            checked={type === "compose"}
            onChange={() => setType("compose")}
          />
          <Icon name="folder" />
          Docker Compose
        </label>
      </fieldset>
      {type === "command" ? (
        <label>
          Run command
          <input
            className="mono"
            required
            value={command}
            placeholder="pnpm dev"
            onChange={(e) => setCommand(e.target.value)}
          />
          <small>
            Runs with your login shell. Use foreground, noninteractive commands.
          </small>
        </label>
      ) : (
        <label>
          Compose file
          <input
            className="mono"
            required
            value={file}
            onChange={(e) => setFile(e.target.value)}
          />
          <small>
            Uses an owned Compose project. Stop preserves containers and
            volumes.
          </small>
        </label>
      )}
      <label>
        Working directory
        <input
          className="mono"
          required
          value={cwd}
          placeholder="."
          onChange={(e) => setCwd(e.target.value)}
        />
        <small>
          Relative to the project folder. Use a dot for the project root.
        </small>
      </label>
      <div className="form-row">
        <label>
          Environment file <span className="optional">optional</span>
          <input
            className="mono"
            value={envFile}
            placeholder=".env.local"
            onChange={(e) => setEnvFile(e.target.value)}
          />
          <small>Path relative to project root. Its contents stay local.</small>
        </label>
        <label>
          Health check <span className="optional">optional</span>
          <input
            className="mono"
            type="url"
            value={health}
            placeholder="http://127.0.0.1:8080/health"
            onChange={(e) => setHealth(e.target.value)}
          />
          <small>Loopback HTTP/HTTPS endpoint.</small>
        </label>
      </div>
      <div className="form-footer">
        <div>
          {onRemove && (
            <button type="button" className="danger-text" onClick={onRemove}>
              Remove service
            </button>
          )}
        </div>
        <div className="button-row">
          <button type="button" className="secondary" onClick={onCancel}>
            Cancel
          </button>
          <button className="primary" disabled={saving}>
            {saving ? "Saving…" : "Save service"}
          </button>
        </div>
      </div>
      <p className="form-note">
        <Icon name="shield" /> Saving does not run your command. Review it
        before the first run.
      </p>
    </form>
  );
}
