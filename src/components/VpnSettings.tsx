import { useState } from "react";
import { api, confirmAction, errorMessage, pickVpnConfig } from "../lib/api";
import type { Project, VpnPortMapping, VpnProfile } from "../lib/types";
import { Icon } from "./Icon";
import "../styles/vpn.css";

const gatewayImage = "qmcgaw/gluetun:v3.41.0";

export function VpnSettings({ profiles, projects, onRefresh }: {
  profiles: VpnProfile[];
  projects: Project[];
  onRefresh: () => Promise<unknown>;
}) {
  const [name, setName] = useState("");
  const [configPath, setConfigPath] = useState("");
  const [authMode, setAuthMode] = useState("password");
  const [keyPassphrase, setKeyPassphrase] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [ports, setPorts] = useState("");
  const [healthTarget, setHealthTarget] = useState("cloudflare.com:443");
  const [dnsAddress, setDnsAddress] = useState("");
  const [healthIcmpTarget, setHealthIcmpTarget] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [formError, setFormError] = useState("");
  const [status, setStatus] = useState("");

  async function perform(action: () => Promise<unknown>) {
    setBusy(true); setError(""); setStatus("");
    try { await action(); await onRefresh(); }
    catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  }

  async function importProfile() {
    const parts = ports.trim() ? ports.split(",").map((part) => part.trim()) : [];
    const publishedPorts: VpnPortMapping[] = [];
    for (const part of parts) {
      const match = /^(\d+):(\d+)$/.exec(part);
      if (!match || match.slice(1).some((value) => Number(value) < 1 || Number(value) > 65535)) {
        setFormError("Use HOST:CONTAINER ports from 1 to 65535, separated by commas."); return;
      }
      publishedPorts.push({ host: Number(match[1]), container: Number(match[2]) });
    }
    if (publishedPorts.length > 32 || new Set(publishedPorts.map((port) => port.host)).size !== publishedPorts.length || new Set(publishedPorts.map((port) => port.container)).size !== publishedPorts.length) {
      setFormError("Use up to 32 unique host and container ports."); return;
    }
    if (publishedPorts.some((port) => [53, 8000, 9999].includes(port.container))) {
      setFormError("Container ports 53, 8000 and 9999 are reserved for VPN DNS, control and health."); return;
    }
    if (!name.trim() || !configPath || (authMode === "password" && (!username.trim() || !password))) {
      setFormError("Choose an .ovpn file and enter a name and the credentials required by the selected authentication method."); return;
    }
    if (!/^[a-zA-Z0-9][a-zA-Z0-9.-]*:[0-9]+$/.test(healthTarget.trim()) || Number(healthTarget.split(":")[1]) < 1 || Number(healthTarget.split(":")[1]) > 65535) {
      setFormError("Use a TCP health endpoint as host:port, with a port from 1 to 65535."); return;
    }
    const ipv4 = (value: string) => /^\d{1,3}(\.\d{1,3}){3}$/.test(value) && value.split(".").every((part) => Number(part) <= 255);
    if ([dnsAddress.trim(), healthIcmpTarget.trim()].some((value) => value && !ipv4(value))) {
      setFormError("Corporate DNS and ICMP health targets must be IPv4 addresses, or blank for defaults."); return;
    }
    setFormError(""); setError(""); setStatus(""); setBusy(true);
    try {
      await api.importVpnProfile({ name: name.trim(), configPath, username: authMode === "password" ? username.trim() : "", password: authMode === "password" ? password : "", ...(keyPassphrase ? { keyPassphrase } : {}), publishedPorts, healthTarget: healthTarget.trim(), ...(dnsAddress.trim() ? { dnsAddress: dnsAddress.trim() } : {}), ...(healthIcmpTarget.trim() ? { healthIcmpTarget: healthIcmpTarget.trim() } : {}) });
      setName(""); setConfigPath(""); setUsername(""); setPassword(""); setKeyPassphrase(""); setPorts(""); setDnsAddress(""); setHealthIcmpTarget("");
      setStatus("VPN profile imported. Credentials are stored privately on this Mac.");
    } catch {
      // Import errors must never reflect private input or config contents.
      setFormError("Could not import profile. Check the .ovpn inline certificates, authentication method, private key passphrase and profile settings.");
      setBusy(false); return;
    }
    try { await onRefresh(); } catch (cause) { setError(errorMessage(cause)); }
    finally { setBusy(false); }
  }

  return <div className="ai-page vpn-page">
    <div className="ai-heading">
      <span className="welcome-icon small"><Icon name="shield" size={24} /></span>
      <h1>Shared Docker VPN</h1>
      <p>Selected Linux service containers share a profile’s tunnel. Separate profiles can connect together. Your Mac stays off VPN.</p>
    </div>
    {error && <p className="runtime-error" role="alert">{error}</p>}
    {status && <p role="status">{status}</p>}
    <section className="settings-section">
      <div className="settings-row"><div><h2>Prepare gateway image</h2><p>Download {gatewayImage} explicitly before connecting. Missing images are never downloaded during a run.</p></div>
        <button className="secondary" disabled={busy} onClick={() => void perform(async () => { await api.pullContainerImage(gatewayImage); setStatus("Gateway image is ready locally."); })}>{busy ? "Working…" : "Download gateway image"}</button>
      </div>
    </section>
    <section className="settings-section">
      <div className="settings-row"><h2>VPN profiles</h2><button className="secondary" disabled={busy} onClick={() => void perform(() => api.listVpnProfiles())}>Refresh profiles</button></div>
      {!profiles.length && <p className="empty-inline">No VPN profiles yet. Import one below, then select it in a command service’s settings.</p>}
      {profiles.map((profile) => {
        const active = profile.users.length > 0;
        const connected = ["ready"].includes(profile.state);
        const transitioning = ["connecting"].includes(profile.state);
        return <article className="vpn-profile" key={profile.id} aria-label={profile.name}>
          <div className="settings-row"><div><h3>{profile.name}</h3><p>{profile.state} · TCP {profile.healthTarget}</p></div>
            <div className="button-row">
              <button className="secondary" disabled={busy || connected || transitioning} onClick={async () => {
                if (await confirmAction(`Connect “${profile.name}” in Docker? Selected services will use this tunnel.`, "Connect VPN?")) void perform(() => api.connectVpn(profile.id));
              }}>Connect</button>
              <button className="secondary" disabled={busy || active || transitioning || profile.state === "disconnected"} onClick={async () => {
                if (await confirmAction(`Disconnect “${profile.name}”?`, "Disconnect VPN?")) void perform(() => api.disconnectVpn(profile.id));
              }}>Disconnect</button>
              <button className="danger-text" disabled={busy || active || transitioning} onClick={async () => {
                if (await confirmAction(`Remove “${profile.name}” and its private saved credentials?`, "Remove VPN profile?")) void perform(() => api.removeVpnProfile(profile.id));
              }}>Remove</button>
            </div>
          </div>
          <p>{profile.publishedPorts.length ? profile.publishedPorts.map((port) => `127.0.0.1:${port.host} → ${port.container}`).join(", ") : "No published host ports"}</p>
          <p>DNS: {profile.dnsAddress || "Gluetun default"} · ICMP health: {profile.healthIcmpTarget || "VPN server (0.0.0.0)"}</p>
          {profile.error && <p className="runtime-error" role="alert">{profile.error}</p>}
          {active ? <><p>In use · Stop these services before disconnecting or removing.</p><ul>{profile.users.map((ref) => {
            const project = projects.find((candidate) => candidate.id === ref.projectId);
            const service = project?.config.services.find((candidate) => candidate.id === ref.serviceId);
            return <li key={`${ref.projectId}/${ref.serviceId}`}>{project?.name ?? ref.projectId} / {service?.name ?? ref.serviceId}</li>;
          })}</ul></> : <p>No active service users</p>}
        </article>;
      })}
    </section>
    <form className="settings-section vpn-import" onSubmit={(event) => { event.preventDefault(); void importProfile(); }}>
      <h2>Import an OpenVPN profile</h2>
      <p>Imported configuration, credentials and any private key passphrase are stored privately by the app, outside project files. The .ovpn must include inline certificates and keys. OTP and browser SSO are unsupported.</p>
      <label>Profile name<input required maxLength={80} value={name} onChange={(event) => setName(event.target.value)} /></label>
      <div className="vpn-file"><span>{configPath ? configPath.split(/[\\/]/).pop() : "No .ovpn file selected"}</span><button type="button" className="secondary" disabled={busy} onClick={() => void perform(async () => {
        const path = await pickVpnConfig(); if (typeof path === "string") setConfigPath(path);
      })}>Choose .ovpn file</button></div>
      <label>Authentication
        <select value={authMode} disabled={busy} onChange={(event) => {
          setAuthMode(event.target.value);
          if (event.target.value === "certificate") { setUsername(""); setPassword(""); }
        }}>
          <option value="password">Username/password</option>
          <option value="certificate">Certificate/key only</option>
        </select>
        <small>Match the .ovpn configuration. Profiles with auth-user-pass require a username and password.</small>
      </label>
      {authMode === "password" && <div className="form-row">
        <label>VPN username<input required autoComplete="off" value={username} onChange={(event) => setUsername(event.target.value)} /></label>
        <label>VPN password<input required type="password" autoComplete="new-password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>
      </div>}
      <label>Private key passphrase <span className="optional">optional</span>
        <input type="password" autoComplete="new-password" value={keyPassphrase} onChange={(event) => setKeyPassphrase(event.target.value)} />
        <small>Required only when the inline private key is encrypted. This is separate from the VPN account password.</small>
      </label>
      <label>TCP health endpoint<input required className="mono" value={healthTarget} onChange={(event) => setHealthTarget(event.target.value)} /><small>Use a reachable host:port, such as a private database on port 5432. Port 443 uses TLS; other ports check TCP connectivity.</small></label>
      <div className="form-row">
        <label>Corporate DNS IPv4 <span className="optional">optional</span><input className="mono" value={dnsAddress} placeholder="10.0.0.53" onChange={(event) => setDnsAddress(event.target.value)} /><small>Blank uses Gluetun’s default resolver. A private DNS IP is used directly by the gateway and workers for corporate hostnames.</small></label>
        <label>ICMP health target IPv4 <span className="optional">optional</span><input className="mono" value={healthIcmpTarget} placeholder="0.0.0.0" onChange={(event) => setHealthIcmpTarget(event.target.value)} /><small>Choose an IPv4 target reachable by ping through this VPN. The private DNS IP can work if it responds. Blank uses the VPN server (0.0.0.0).</small></label>
      </div>
      <label>Published ports <span className="optional">optional</span><input className="mono" value={ports} placeholder="3000:3000, 8080:8080" onChange={(event) => setPorts(event.target.value)} /><small>HOST:CONTAINER, comma-separated. Host listeners bind only to 127.0.0.1; configure these shared mappings before connecting. Container ports 53, 8000 and 9999 are reserved.</small></label>
      {formError && <p className="field-error" role="alert">{formError}</p>}
      <button className="primary" disabled={busy}>{busy ? "Working…" : "Import profile"}</button>
    </form>
  </div>;
}
