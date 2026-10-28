import { useState } from "react";
import type { ServiceDefinition, VpnProfile } from "../lib/types";
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
  autoFocusName = true,
}: {
  service?: ServiceDefinition;
  autoFocusName?: boolean;
  vpnProfiles?: VpnProfile[];
  services?: ServiceEntry[];
  projectId?: string;
  saving: boolean;
  onSave: (service: ServiceDefinition) => void;
  onCancel: () => void;
  onRemove?: () => void;
}) {
  const [vpnRoutes, setVpnRoutes] = useState(() =>
    service?.type === "command"
      ? (service.vpnRoutes ?? []).map((route) => ({
          profileId: route.profileId,
          remoteHost: route.remoteHost,
          remotePort: String(route.remotePort),
          localPort: String(route.localPort),
        }))
      : [],
  );
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
  const [vpnRouteError, setVpnRouteError] = useState("");
  return (
    <form
      className="service-form"
      title="Save service: ⌘S / Ctrl+S"
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
        const parsedRoutes = vpnRoutes.map((route) => ({
          profileId: route.profileId,
          remoteHost: route.remoteHost.trim(),
          remotePort: Number(route.remotePort),
          localPort: Number(route.localPort),
        }));
        const savedProfileIds = new Set(
          service?.type === "command"
            ? service.vpnRoutes?.map((route) => route.profileId) ?? []
            : [],
        );
        const invalidRoutes = parsedRoutes.some((route) =>
          (!vpnProfiles.some((profile) => profile.id === route.profileId) &&
            !savedProfileIds.has(route.profileId)) ||
          !/^[A-Za-z0-9](?:[A-Za-z0-9._-]*[A-Za-z0-9])?$/.test(route.remoteHost) ||
          !Number.isInteger(route.remotePort) || route.remotePort < 1 || route.remotePort > 65535 ||
          !Number.isInteger(route.localPort) || route.localPort < 1 || route.localPort > 65535
        ) || new Set(parsedRoutes.map((route) => route.localPort)).size !== parsedRoutes.length;
        setVpnRouteError(invalidRoutes ? "Choose a VPN, valid hostname, unique local port, and TCP ports from 1 to 65535." : "");
        if (invalidPorts || invalidUrl || invalidRoutes) return;
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
            ? { ...common, type, command: command.trim(), ...(parsedRoutes.length ? { vpnRoutes: parsedRoutes } : {}) }
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
      <section className="form-section" aria-label="Service configuration">
      <div className="form-row">
        <label>
          Service name
          <input
            autoFocus={autoFocusName}
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
      {type === "command" && <fieldset className="vpn-options">
        <legend>Network</legend>
        <small>The service command still runs natively. Point only the matching app endpoint at 127.0.0.1 and its local relay port.</small>
        {vpnRoutes.map((route, index) => <fieldset className="vpn-route" key={index}>
          <legend>VPN route {index + 1}</legend>
          <label>VPN profile {index + 1}
            <select value={route.profileId} onChange={(event) => setVpnRoutes((routes) => routes.map((entry, current) => current === index ? { ...entry, profileId: event.target.value } : entry))}>
              <option value="">Choose VPN</option>
              {route.profileId && !vpnProfiles.some((profile) => profile.id === route.profileId) && <option value={route.profileId} disabled>Unavailable profile ({route.profileId})</option>}
              {vpnProfiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name} · {profile.state}</option>)}
            </select>
          </label>
          <label>Destination host {index + 1}
            <input className="mono" value={route.remoteHost} placeholder="database.internal.example" onChange={(event) => setVpnRoutes((routes) => routes.map((entry, current) => current === index ? { ...entry, remoteHost: event.target.value } : entry))} />
          </label>
          <label>Remote port {index + 1}
            <input className="mono" inputMode="numeric" value={route.remotePort} placeholder="5432" onChange={(event) => setVpnRoutes((routes) => routes.map((entry, current) => current === index ? { ...entry, remotePort: event.target.value } : entry))} />
          </label>
          <label>Local port {index + 1}
            <input className="mono" inputMode="numeric" value={route.localPort} placeholder="15432" onChange={(event) => setVpnRoutes((routes) => routes.map((entry, current) => current === index ? { ...entry, localPort: event.target.value } : entry))} />
          </label>
          <button type="button" className="secondary" aria-label={`Remove VPN route ${index + 1}`} onClick={() => setVpnRoutes((routes) => routes.filter((_, current) => current !== index))}>Remove</button>
        </fieldset>)}
        <button type="button" className="secondary" onClick={() => setVpnRoutes((routes) => [...routes, { profileId: vpnProfiles[0]?.id ?? "", remoteHost: "", remotePort: "", localPort: "" }])}>Add VPN route</button>
        {vpnRouteError && <small className="field-error" role="alert">{vpnRouteError}</small>}
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
      </section>
      <div className="form-footer form-actions" role="group" aria-label="Form actions">
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
