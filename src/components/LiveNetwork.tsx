import { useState } from "react";
import { useNetworkTraffic } from "../lib/useNetworkTraffic";
import type { Workspace } from "../lib/types";

export function LiveNetwork({ workspace }: { workspace: Workspace }) {
  const { samples, activity, loading, error } = useNetworkTraffic();
  const [selectedKey, setSelectedKey] = useState("");
  const routes = workspace.projects.flatMap((project) => project.config.services.flatMap((service) =>
    service.type === "command" ? (service.vpnRoutes ?? []).map((route) => {
      const key = JSON.stringify([project.id, service.id, route.localPort, route.profileId, route.remoteHost, route.remotePort]);
      const runId = project.runtime[service.id]?.runId;
      const state = project.runtime[service.id]?.state ?? "stopped";
      const active = ["starting", "running", "ready"].includes(state);
      const sample = active && !error ? samples.find((item) => item.projectId === project.id && item.serviceId === service.id && item.runId === runId && item.localPort === route.localPort && item.profileId === route.profileId && item.remoteHost === route.remoteHost && item.remotePort === route.remotePort) : undefined;
      const pulse = sample && !error ? activity[`${project.id}/${service.id}/${sample.runId}/${sample.localPort}`] : undefined;
      const tx = !!pulse?.tx && active;
      const rx = !!pulse?.rx && active;
      const status = !active ? "Stopped" : error ? "Unavailable" : !sample ? "Awaiting sample" : tx && rx ? "Sending and receiving" : tx ? "Sending" : rx ? "Receiving" : "Idle";
      return { key, project, service, route, sample, active, tx, rx, status, profile: workspace.vpns?.find((vpn) => vpn.id === route.profileId)?.name ?? route.profileId };
    }) : []));
  const unsupported = workspace.projects.flatMap((project) => project.config.services.filter((service) => service.type !== "command" || !service.vpnRoutes?.length).map((service) => ({ project, service })));
  const observed = routes.flatMap((route) => route.sample ? [route.sample] : []);
  const selected = routes.find((route) => route.key === selectedKey) ?? routes[0];
  if (selectedKey && !routes.some((route) => route.key === selectedKey)) setSelectedKey(selected?.key ?? "");
  const activeCount = routes.filter((route) => route.active).length;
  const covered = !loading && !error && activeCount > 0 && observed.length === activeCount;
  const coverage = error ? "Unavailable" : loading ? "Loading…" : activeCount ? `${observed.length} of ${activeCount} active routes observed` : "No active routes";

  return <section className="live-network">
    <header className="network-header"><div><h1>Live Network</h1><p>Follow configured service routes through VPN profiles to their destinations.</p></div><span>{coverage}</span></header>
    <div className="network-summary"><div><strong>{error || loading ? "—" : observed.length}</strong><span>Observable routes</span></div><div><strong>{error || loading ? "—" : routes.reduce((sum, route) => sum + Number(route.tx) + Number(route.rx), 0)}</strong><span>Pulsing directions</span></div><div><strong>{!covered ? "—" : observed.reduce((sum, sample) => sum + sample.activeConnections, 0)}</strong><span>Open connections</span></div><div><strong>{!covered ? "—" : `${observed.reduce((sum, sample) => sum + sample.txBytes, 0).toLocaleString()} B`}</strong><span>TX total</span></div><div><strong>{!covered ? "—" : `${observed.reduce((sum, sample) => sum + sample.rxBytes, 0).toLocaleString()} B`}</strong><span>RX total</span></div></div>
    <div className="network-graph-scroll" tabIndex={0} aria-label="Configured routes, scroll horizontally">
      <div className="network-graph"><div className="network-columns" aria-hidden="true"><span>This Mac</span><span>Services</span><span>Network path</span><span>Destinations</span></div>
        {routes.length === 0 && <p className="network-empty">No VPN routes configured.</p>}
        {routes.map((item) => <div className="network-row" key={item.key}>
          <svg className="network-lines" viewBox="0 0 900 80" preserveAspectRatio="none" aria-hidden="true"><path d="M0 40 H900" />{item.tx && <circle className="network-pulse tx" r="4"><animateMotion dur="1.5s" repeatCount="indefinite" path="M0 36 H900" /></circle>}{item.rx && <circle className="network-pulse rx" r="4"><animateMotion dur="1.5s" repeatCount="indefinite" path="M900 44 H0" /></circle>}</svg>
          <span className="network-node network-mac">This Mac</span><button className="network-node network-route" aria-pressed={selected?.key === item.key} aria-label={`${item.service.name}, ${item.project.name}, local ${item.route.localPort}, ${item.profile}, ${item.route.remoteHost}:${item.route.remotePort}, ${item.status}`} onClick={() => setSelectedKey(item.key)}><strong title={item.service.name}>{item.service.name}</strong><span>{item.project.name} · Local {item.route.localPort}</span><span title={`${item.route.remoteHost}:${item.route.remotePort}`}>{item.route.remoteHost}:{item.route.remotePort}</span><span>{item.status}</span></button><span className="network-node"><strong title={item.profile}>{item.profile}</strong><small>VPN profile</small></span><span className="network-node"><strong title={`${item.route.remoteHost}:${item.route.remotePort}`}>{item.route.remoteHost}:{item.route.remotePort}</strong><small>Destination</small></span>
        </div>)}
      </div>
    </div>
    {selected && <section className="network-detail" aria-label="Route details"><div><h2>{selected.service.name} → {selected.route.remoteHost}:{selected.route.remotePort}</h2><p>Through {selected.profile} · {selected.project.name}</p></div><dl><div><dt>Status</dt><dd>{selected.status}</dd></div><div><dt>Local port</dt><dd>{selected.route.localPort}</dd></div><div><dt>TX</dt><dd>{selected.sample ? `${selected.sample.txBytes.toLocaleString()} B` : "—"}</dd></div><div><dt>RX</dt><dd>{selected.sample ? `${selected.sample.rxBytes.toLocaleString()} B` : "—"}</dd></div><div><dt>Connections</dt><dd>{selected.sample?.activeConnections ?? "—"}</dd></div></dl></section>}
    {unsupported.length > 0 && <section className="network-unsupported"><h2>Not observable</h2><p>These services have no configured native VPN routes.</p><ul>{unsupported.map(({ project, service }) => <li key={JSON.stringify([project.id, service.id])}><strong>{service.name}</strong> <span>{project.name} · Not observable</span></li>)}</ul></section>}
  </section>;
}
