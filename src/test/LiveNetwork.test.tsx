import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { LiveNetwork } from "../components/LiveNetwork";
import type { Workspace, NetworkTraffic } from "../lib/types";

const traffic = vi.hoisted(() => ({ samples: [] as NetworkTraffic[], activity: {} as Record<string, { tx: boolean; rx: boolean }>, loading: false, error: null as string | null }));
vi.mock("../lib/useNetworkTraffic", () => ({ useNetworkTraffic: () => traffic }));
const workspace: Workspace = {
  projects: [{ id: "p", name: "Project", path: "/project", revision: "1", trusted: [], runtime: { a: { state: "running", runId: "run", health: "healthy" }, b: { state: "stopped", health: "notConfigured" } }, config: { schemaVersion: 1, services: [
    { id: "a", name: "API", type: "command", command: "run", cwd: "/project", vpnRoutes: [{ profileId: "vpn", localPort: 1001, remoteHost: "db.local", remotePort: 5432 }, { profileId: "vpn", localPort: 1002, remoteHost: "db.local", remotePort: 5432 }] },
    { id: "b", name: "Worker", type: "command", command: "run", cwd: "/project", vpnRoutes: [{ profileId: "vpn", localPort: 1003, remoteHost: "other.local", remotePort: 443 }] },
    { id: "c", name: "Direct", type: "command", command: "run", cwd: "/project" },
    { id: "d", name: "Docker", type: "compose", file: "compose.yml", cwd: "/project" },
  ] } }], groups: [], vpns: [{ id: "vpn", name: "Work VPN", publishedPorts: [], outboundSubnets: [], healthTarget: "", state: "running", users: [] }], ai: { enabled: false, endpoint: "", clients: [] }, uncleanShutdown: false,
};
const sample: NetworkTraffic = { projectId: "p", serviceId: "a", runId: "run", profileId: "vpn", localPort: 1001, remoteHost: "db.local", remotePort: 5432, txBytes: 20, rxBytes: 10, activeConnections: 1 };
beforeEach(() => { traffic.samples = []; traffic.activity = {}; traffic.error = null; });

it("keeps configured and stopped routes neutral, and identifies unsupported services", () => {
  traffic.samples = [sample];
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getAllByRole("button", { name: /db.local:5432/ })).toHaveLength(2);
  expect(screen.getAllByRole("button", { name: /db.local:5432/ })[0]).toHaveTextContent("Idle");
  expect(screen.getByText("Stopped")).toBeVisible();
  expect(screen.getByText("Direct").closest("li")).toHaveTextContent("Not observable");
  expect(screen.getByText("Docker").closest("li")).toHaveTextContent("Not observable");
  expect(document.querySelectorAll(".network-pulse")).toHaveLength(0);
});

it("shows TX and RX activity and decorative direction for the matching route", () => {
  traffic.samples = [sample]; traffic.activity = { "p/a/run/1001": { tx: true, rx: true } };
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getAllByRole("button", { name: /db.local:5432/ })[0]).toHaveTextContent("Sending and receiving");
  expect(document.querySelector("svg[aria-hidden=true] .network-pulse.tx")).toBeInTheDocument();
  expect(document.querySelector("svg[aria-hidden=true] .network-pulse.rx")).toBeInTheDocument();
});

it("selects distinct routes by keyboard and repairs selection after removal", async () => {
  const { rerender } = render(<LiveNetwork workspace={workspace} />);
  const buttons = screen.getAllByRole("button", { name: /db.local:5432/ });
  buttons[1].focus(); await userEvent.keyboard("{Enter}");
  expect(within(screen.getByLabelText("Route details")).getByText("1002")).toBeVisible();
  const changed = structuredClone(workspace);
  const service = changed.projects[0].config.services[0];
  if (service.type === "command") service.vpnRoutes = service.vpnRoutes?.slice(0, 1);
  rerender(<LiveNetwork workspace={changed} />);
  expect(within(screen.getByLabelText("Route details")).getByText("1001")).toBeVisible();
});

it("shows unavailable without discarding configured routes on polling error", () => {
  traffic.error = "offline";
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getAllByRole("button", { name: /db.local:5432/ })[0]).toHaveTextContent("Unavailable");
  expect(screen.getAllByRole("button", { name: /db.local:5432/ })).toHaveLength(2);
});

it.each([[true, false, "Sending"], [false, true, "Receiving"]] as const)("shows single direction activity", (tx, rx, label) => {
  traffic.samples = [sample]; traffic.activity = { "p/a/run/1001": { tx, rx } };
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getAllByRole("button", { name: /db.local:5432/ })[0]).toHaveTextContent(label);
  expect(document.querySelectorAll(".network-pulse")).toHaveLength(1);
  expect(document.querySelector(`.network-pulse.${tx ? "tx" : "rx"}`)).toBeInTheDocument();
});

it("does not show stale counters or pulses for a stopped route", () => {
  traffic.samples = [sample]; traffic.activity = { "p/a/run/1001": { tx: true, rx: true } };
  const stoppedWorkspace = structuredClone(workspace);
  stoppedWorkspace.projects[0].runtime.a.state = "stopped";
  render(<LiveNetwork workspace={stoppedWorkspace} />);
  expect(screen.getAllByRole("button", { name: /db.local:5432/ })[0]).toHaveTextContent("Stopped");
  expect(document.querySelectorAll(".network-pulse")).toHaveLength(0);
  expect(within(screen.getByLabelText("Route details")).getAllByText("—")).toHaveLength(3);
});

it("does not present zero as a measured total when polling fails", () => {
  traffic.error = "offline";
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getByText("Open connections").parentElement).toHaveTextContent("—");
});

it("counts current observable routes and pulsing directions, excluding stopped and missing samples", () => {
  traffic.samples = [sample, { ...sample, serviceId: "b", localPort: 1003, remoteHost: "other.local", remotePort: 443 }];
  traffic.activity = { "p/a/run/1001": { tx: true, rx: true }, "p/b/run/1003": { tx: true, rx: true } };
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getByText("Observable routes").parentElement).toHaveTextContent("1");
  expect(screen.getByText("Pulsing directions").parentElement).toHaveTextContent("2");
});

it("includes the resolved profile in each duplicate destination route name", () => {
  const changed = structuredClone(workspace);
  const service = changed.projects[0].config.services[0];
  if (service.type === "command" && service.vpnRoutes) service.vpnRoutes[1].profileId = "vpn2";
  changed.vpns?.push({ id: "vpn2", name: "Other VPN", publishedPorts: [], outboundSubnets: [], healthTarget: "", state: "running", users: [] });
  render(<LiveNetwork workspace={changed} />);
  expect(screen.getByRole("button", { name: /Work VPN.*db.local:5432/ })).toBeVisible();
  expect(screen.getByRole("button", { name: /Other VPN.*db.local:5432/ })).toBeVisible();
});

it("keeps the repaired selection when a removed route returns", async () => {
  const { rerender } = render(<LiveNetwork workspace={workspace} />);
  await userEvent.click(screen.getAllByRole("button", { name: /db.local:5432/ })[1]);
  const changed = structuredClone(workspace);
  const service = changed.projects[0].config.services[0];
  if (service.type === "command") service.vpnRoutes = service.vpnRoutes?.slice(0, 1);
  rerender(<LiveNetwork workspace={changed} />);
  rerender(<LiveNetwork workspace={workspace} />);
  expect(within(screen.getByLabelText("Route details")).getByText("1001")).toBeVisible();
});

it("provides full titles and accessible route name for long labels", () => {
  const changed = structuredClone(workspace);
  changed.projects[0].config.services[0].name = "Long service name that exceeds the card width";
  if (changed.vpns) changed.vpns[0].name = "Long VPN profile name that exceeds the card width";
  const service = changed.projects[0].config.services[0];
  if (service.type === "command" && service.vpnRoutes) service.vpnRoutes[0].remoteHost = "very-long-destination-name.example.internal";
  render(<LiveNetwork workspace={changed} />);
  expect(screen.getAllByTitle("Long service name that exceeds the card width")[0]).toBeVisible();
  expect(screen.getAllByTitle("Long VPN profile name that exceeds the card width")[0]).toBeVisible();
  expect(screen.getAllByTitle("very-long-destination-name.example.internal:5432")).toHaveLength(2);
  expect(screen.getByRole("button", { name: /Long service name.*Long VPN profile name.*very-long-destination-name/ })).toBeVisible();
});

it("marks partial sample coverage and does not present missing routes as measured totals", () => {
  traffic.samples = [sample];
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getByText("1 of 2 active routes observed")).toBeVisible();
  expect(screen.getByText("Open connections").parentElement).toHaveTextContent("—");
});

it.each([[true, false], [false, true]] as const)("counts a single active direction", (tx, rx) => {
  traffic.samples = [sample]; traffic.activity = { "p/a/run/1001": { tx, rx } };
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getByText("Pulsing directions").parentElement).toHaveTextContent("1");
});

it("reports no samples while active routes await observation", () => {
  render(<LiveNetwork workspace={workspace} />);
  expect(screen.getByText("0 of 2 active routes observed")).toBeVisible();
  expect(screen.getByText("Open connections").parentElement).toHaveTextContent("—");
  expect(screen.getByText("TX total").parentElement).toHaveTextContent("—");
  expect(screen.getByText("RX total").parentElement).toHaveTextContent("—");
});
