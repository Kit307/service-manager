import { useState } from "react";
import type { ServiceDefinition, VpnProfile } from "../lib/types";
import { api, errorMessage } from "../lib/api";
import type { ServiceEntry } from "../lib/workspace";
import "../styles/operations.css";
import "../styles/vpn.css";
import { Icon } from "./Icon";

export function ServiceEditor({
  service,
  saving,
  onSave,
  onCancel,
  onRemove,
  services,
  projectId,
  vpnProfiles = [],
}: {
  service?: ServiceDefinition;
  vpnProfiles?: VpnProfile[];
  services?: ServiceEntry[];
  projectId?: string;
  saving: boolean;
  onSave: (service: ServiceDefinition) => void;
  onCancel: () => void;
  onRemove?: () => void;
}) {
  const [profileId, setProfileId] = useState(service?.type === "command" ? service.vpn?.profileId ?? "" : "");
  const [image, setImage] = useState(service?.type === "command" ? service.vpn?.image ?? "" : "");
  const [pulling, setPulling] = useState(false);
  const [imageError, setImageError] = useState("");
  const [imageStatus, setImageStatus] = useState("");
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
  const [ports, setPorts] = useState(service?.ports?.join(", ") ?? "");
  const [url, setUrl] = useState(service?.url ?? "");
  const [dependsOn, setDependsOn] = useState(service?.dependsOn ?? []);
  const [portsError, setPortsError] = useState("");
  const [urlError, setUrlError] = useState("");
  return (
    <form
      className="service-form"
      onSubmit={(event) => {
        event.preventDefault();
        const parts = ports.trim() ? ports.split(",").map((part) => part.trim()) : [];
        const parsedPorts = parts.map(Number);
        const invalidPorts =
          parts.some((part) => !/^\d+$/.test(part)) ||
          parsedPorts.some((port) => !Number.isInteger(port) || port < 1 || port > 65535) ||
          new Set(parsedPorts).size !== parsedPorts.length;
        setPortsError(invalidPorts ? "Enter unique TCP ports from 1 to 65535, separated by commas." : "");
        let invalidUrl = false;
        if (url.trim()) {
          try {
            const parsed = new URL(url.trim());
            invalidUrl =
              !["http:", "https:"].includes(parsed.protocol) ||
              !["localhost", "127.0.0.1", "[::1]"].includes(parsed.hostname) ||
              !!parsed.username || !!parsed.password;
          } catch {
            invalidUrl = true;
          }
        }
        setUrlError(invalidUrl ? "Use a loopback HTTP/HTTPS URL without credentials." : "");
        if (invalidPorts || invalidUrl) return;
        const common = {
          id,
          name: name.trim(),
          cwd: cwd.trim() || ".",
          ...(parsedPorts.length ? { ports: parsedPorts } : {}),
          ...(url.trim() ? { url: url.trim() } : {}),
          ...(dependsOn.length ? { dependsOn } : {}),
          ...(envFile.trim() ? { envFile: envFile.trim() } : {}),
          ...(health.trim() ? { healthCheck: { url: health.trim() } } : {}),
        };
        onSave(
          type === "command"
            ? { ...common, type, command: command.trim(), ...(profileId ? { vpn: { profileId, image: image.trim() } } : {}) }
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
          <Icon name="stack" />
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
            {profileId ? "Runs in a Linux container through the selected VPN. Edit host-specific absolute paths and commands for Linux." : "Runs with your login shell. Use foreground, noninteractive commands."}
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
      {type === "command" && <fieldset className="vpn-options">
        <legend>Network</legend>
        <label>VPN profile
          <select value={profileId} onChange={(event) => {
            setProfileId(event.target.value);
            if (event.target.value && !image) setImage("node:22-bookworm-slim");
          }}>
            <option value="">None · Native on this Mac</option>
            {profileId && !vpnProfiles.some((profile) => profile.id === profileId) && <option value={profileId} disabled>Unavailable profile ({profileId})</option>}
            {vpnProfiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name} · {profile.state}</option>)}
          </select>
        </label>
        {profileId && <>
          <label>Linux runtime image
            <input required disabled={pulling} className="mono" value={image} placeholder="node:22-bookworm-slim" onChange={(event) => { setImage(event.target.value); setImageError(""); setImageStatus(""); }} />
          </label>
          <small>Use an image with your runtime, such as node:22-bookworm-slim or golang:1.24-bookworm. The working folder is mounted in Linux; Mac absolute paths do not work. Bind the service to 0.0.0.0. Configure published host ports on the VPN page; Ports below only inspect host listeners.</small>
          <button type="button" className="secondary" disabled={pulling || !image.trim()} onClick={async () => {
            setPulling(true); setImageError(""); setImageStatus("");
            try { await api.pullContainerImage(image.trim()); setImageStatus("Runtime image is ready locally."); }
            catch (error) { setImageError(errorMessage(error)); }
            finally { setPulling(false); }
          }}>{pulling ? "Downloading image…" : "Download runtime image"}</button>
          <small>Images must exist locally before running. Downloads only start when you request them.</small>
          {imageError && <small className="field-error" role="alert">{imageError}</small>}
          {imageStatus && <small role="status">{imageStatus}</small>}
        </>}
      </fieldset>}
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
          Relative to the working folder above. Use a dot for its root.
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
          <small>Path relative to the working folder. Its contents stay local.</small>
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
      <div className="form-row">
        <label>
          Ports <span className="optional">optional</span>
          <input
            className="mono"
            value={ports}
            placeholder="3000, 8080"
            aria-invalid={!!portsError}
            aria-describedby={portsError ? "ports-error" : undefined}
            onChange={(event) => { setPorts(event.target.value); setPortsError(""); }}
          />
          <small>Comma-separated TCP ports used for listener diagnostics.</small>
          {portsError && <small id="ports-error" className="field-error" role="alert">{portsError}</small>}
        </label>
        <label>
          Open URL <span className="optional">optional</span>
          <input
            className="mono"
            value={url}
            placeholder="http://127.0.0.1:3000"
            aria-invalid={!!urlError}
            aria-describedby={urlError ? "open-url-error" : undefined}
            onChange={(event) => { setUrl(event.target.value); setUrlError(""); }}
          />
          <small>Loopback HTTP/HTTPS page to open. Separate from the health check.</small>
          {urlError && <small id="open-url-error" className="field-error" role="alert">{urlError}</small>}
        </label>
      </div>
      {services && (
        <fieldset className="dependency-options">
          <legend>Dependencies</legend>
          <small>Dependencies wait until Ready and require a configured health check.</small>
          {services.filter((entry) => entry.project.id !== projectId || entry.service.id !== id).map((entry) => {
            const ref = { projectId: entry.project.id, serviceId: entry.service.id };
            const checked = dependsOn.some((dependency) => dependency.projectId === ref.projectId && dependency.serviceId === ref.serviceId);
            return (
              <label key={JSON.stringify(ref)}>
                <input type="checkbox" checked={checked} onChange={(event) => setDependsOn((old) =>
                  event.target.checked ? [...old, ref] : old.filter((dependency) => dependency.projectId !== ref.projectId || dependency.serviceId !== ref.serviceId),
                )} />
                {entry.service.name} · {entry.project.name}{!entry.service.healthCheck && " (health check required)"}
              </label>
            );
          })}
        </fieldset>
      )}
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
