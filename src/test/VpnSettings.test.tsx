import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { api, confirmAction, pickVpnConfig } from "../lib/api";
import { VpnSettings } from "../components/VpnSettings";
import { ServiceEditor } from "../components/ServiceEditor";
import type { ServiceDefinition, VpnProfile } from "../lib/types";

vi.mock("../lib/api", async (original) => ({
  ...await original<typeof import("../lib/api")>(),
  confirmAction: vi.fn(), pickVpnConfig: vi.fn(),
  api: { respondVpnChallenge: vi.fn(), importVpnProfile: vi.fn(), updateVpnNetwork: vi.fn(), connectVpn: vi.fn(), disconnectVpn: vi.fn(), removeVpnProfile: vi.fn(), listVpnProfiles: vi.fn(), pullContainerImage: vi.fn() },
}));
const profile: VpnProfile = { id: "fixture-vpn", name: "Fixture VPN", healthTarget: "cloudflare.com:443", publishedPorts: [{ host: 3000, container: 8080 }], outboundSubnets: [], state: "ready", users: [] };
const service: ServiceDefinition = { id: "fixture", name: "Fixture", type: "command", command: "npm start", cwd: "." };
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(pickVpnConfig).mockResolvedValue("/private/fixture.ovpn");
  vi.mocked(api.importVpnProfile).mockResolvedValue(profile);
  vi.mocked(confirmAction).mockResolvedValue(true);
});
async function fillImport() {
  await userEvent.type(screen.getByLabelText("Profile name"), "Fixture VPN");
  await userEvent.click(screen.getByRole("button", { name: "Choose .ovpn file" }));
  await screen.findByText("fixture.ovpn");
  await userEvent.type(screen.getByLabelText("VPN username"), "synthetic-user");
  await userEvent.type(screen.getByLabelText("VPN password"), "synthetic-password");
}
it("validates ports and private TLS endpoint before importing, then clears credentials", async () => {
  const refresh = vi.fn();
  render(<VpnSettings profiles={[]} projects={[]} onRefresh={refresh} />);
  await fillImport();
  const ports = screen.getByLabelText(/Published ports/);
  await userEvent.type(ports, "3000:8080,3000:9000");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  expect(screen.getByRole("alert")).toHaveTextContent("unique");
  expect(api.importVpnProfile).not.toHaveBeenCalled();
  await userEvent.clear(ports); await userEvent.type(ports, "3000:53");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  expect(screen.getByRole("alert")).toHaveTextContent("reserved");
  expect(api.importVpnProfile).not.toHaveBeenCalled();
  await userEvent.clear(ports); await userEvent.type(ports, "3000:11940");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  expect(screen.getByRole("alert")).toHaveTextContent("11940");
  expect(api.importVpnProfile).not.toHaveBeenCalled();
  await userEvent.clear(ports); await userEvent.type(ports, "3000:8080");
  const health = screen.getByLabelText(/TCP health endpoint/);
  await userEvent.clear(health); await userEvent.type(health, "https://private.invalid");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  expect(api.importVpnProfile).not.toHaveBeenCalled();
  await userEvent.clear(health); await userEvent.type(health, "private.invalid:443");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  await waitFor(() => expect(refresh).toHaveBeenCalled());
  expect(api.importVpnProfile).toHaveBeenCalledWith({ name: "Fixture VPN", configPath: "/private/fixture.ovpn", username: "synthetic-user", password: "synthetic-password", publishedPorts: [{ host: 3000, container: 8080 }], healthTarget: "private.invalid:443" });
  expect(screen.getByLabelText("VPN username")).toHaveValue("");
  expect(screen.getByLabelText("VPN password")).toHaveValue("");
  expect(screen.queryByText("fixture.ovpn")).not.toBeInTheDocument();
  expect(screen.getByRole("status")).not.toHaveTextContent("synthetic-password");
});
it("preserves private form input on import failure without reflecting backend error contents", async () => {
  vi.mocked(api.importVpnProfile).mockRejectedValue(new Error("synthetic-password <key>private material</key>"));
  render(<VpnSettings profiles={[]} projects={[]} onRefresh={vi.fn()} />);
  await fillImport();
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not import profile");
  expect(screen.getByRole("alert")).not.toHaveTextContent("synthetic-password");
  expect(screen.getByLabelText("VPN password")).toHaveValue("synthetic-password");
  expect(screen.getByLabelText("VPN username")).toHaveValue("synthetic-user");
  expect(screen.getByText("fixture.ovpn")).toBeVisible();
});
it("blocks disconnect/removal for active users and confirms inactive actions", async () => {
  const view = render(<VpnSettings profiles={[{ ...profile, users: [{ projectId: "fixture-project", serviceId: "fixture" }] }]} projects={[]} onRefresh={vi.fn()} />);
  const row = within(screen.getByRole("article", { name: "Fixture VPN" }));
  expect(row.getByRole("button", { name: "Disconnect" })).toBeDisabled();
  expect(row.getByRole("button", { name: "Remove" })).toBeDisabled();
  expect(row.getByText("fixture-project / fixture")).toBeVisible();
  view.rerender(<VpnSettings profiles={[profile]} projects={[]} onRefresh={vi.fn()} />);
  await userEvent.click(row.getByRole("button", { name: "Disconnect" }));
  await waitFor(() => expect(api.disconnectVpn).toHaveBeenCalledWith(profile.id));
  await userEvent.click(row.getByRole("button", { name: "Remove" }));
  await waitFor(() => expect(api.removeVpnProfile).toHaveBeenCalledWith(profile.id));
  expect(confirmAction).toHaveBeenCalledTimes(2);
});
it("downloads the pinned gateway only on explicit request and preserves useful errors", async () => {
  vi.mocked(api.pullContainerImage).mockRejectedValue(new Error("Docker is unavailable"));
  render(<VpnSettings profiles={[{ ...profile, state: "disconnected", error: "Image missing: download gateway image" }]} projects={[]} onRefresh={vi.fn()} />);
  expect(api.pullContainerImage).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent("Image missing");
  await userEvent.click(screen.getByRole("button", { name: "Download gateway image" }));
  expect(api.pullContainerImage).toHaveBeenCalledWith("qmcgaw/gluetun:v3.41.0");
  expect(await screen.findByText("Docker is unavailable")).toBeVisible();
});
it("edits ports and private outbound CIDRs on a disconnected profile without reimporting credentials", async () => {
  const refresh = vi.fn();
  vi.mocked(api.updateVpnNetwork).mockResolvedValue({ ...profile, publishedPorts: [{ host: 3000, container: 3000 }], outboundSubnets: ["192.168.5.0/24"] });
  render(<VpnSettings profiles={[{ ...profile, state: "disconnected", publishedPorts: [] }]} projects={[]} onRefresh={refresh} />);
  const row = within(screen.getByRole("article", { name: "Fixture VPN" }));
  await userEvent.type(row.getByLabelText("Published ports"), "3000:3000");
  await userEvent.type(row.getByLabelText(/Local outbound CIDRs/), "192.168.5.0/24");
  await userEvent.click(row.getByRole("button", { name: "Save network" }));
  await waitFor(() => expect(api.updateVpnNetwork).toHaveBeenCalledWith(profile.id, [{ host: 3000, container: 3000 }], ["192.168.5.0/24"]));
  expect(api.importVpnProfile).not.toHaveBeenCalled();
  expect(refresh).toHaveBeenCalled();
});
it("preserves absent vpn for native and Compose services", async () => {
  const save = vi.fn();
  render(<ServiceEditor service={service} saving={false} onSave={save} onCancel={vi.fn()} />);
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith(service);
  await userEvent.click(screen.getByLabelText("Docker Compose"));
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith({ id: "fixture", name: "Fixture", type: "compose", file: "compose.yaml", cwd: "." });
});
it("saves multiple VPN destination routes without changing the native command", async () => {
  const save = vi.fn();
  const second = { ...profile, id: "fixture-vpn-2", name: "Second VPN" };
  render(<ServiceEditor service={service} vpnProfiles={[profile, second]} saving={false} onSave={save} onCancel={vi.fn()} />);
  await userEvent.click(screen.getByRole("button", { name: "Add VPN route" }));
  await userEvent.selectOptions(screen.getByLabelText("VPN profile 1"), profile.id);
  await userEvent.type(screen.getByLabelText("Destination host 1"), "sso.internal.example");
  await userEvent.type(screen.getByLabelText("Remote port 1"), "443");
  await userEvent.type(screen.getByLabelText("Local port 1"), "18443");
  await userEvent.click(screen.getByRole("button", { name: "Add VPN route" }));
  await userEvent.selectOptions(screen.getByLabelText("VPN profile 2"), second.id);
  await userEvent.type(screen.getByLabelText("Destination host 2"), "database.internal.example");
  await userEvent.type(screen.getByLabelText("Remote port 2"), "5432");
  await userEvent.type(screen.getByLabelText("Local port 2"), "15432");
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith({ ...service, vpnRoutes: [
    { profileId: profile.id, remoteHost: "sso.internal.example", remotePort: 443, localPort: 18443 },
    { profileId: second.id, remoteHost: "database.internal.example", remotePort: 5432, localPort: 15432 },
  ] });
  expect(screen.getByText(/service command still runs natively/i)).toBeVisible();
  await userEvent.click(screen.getByLabelText("Docker Compose"));
  expect(screen.queryByRole("button", { name: "Add VPN route" })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save.mock.lastCall?.[0]).not.toHaveProperty("vpnRoutes");
});
it("preserves a missing saved route profile until the route is removed", async () => {
  const save = vi.fn();
  const saved: ServiceDefinition = { ...service, vpnRoutes: [{ profileId: "missing", remoteHost: "db.internal.example", remotePort: 5432, localPort: 15432 }] };
  render(<ServiceEditor service={saved} vpnProfiles={[]} saving={false} onSave={save} onCancel={vi.fn()} />);
  expect(screen.getByRole("option", { name: "Unavailable profile (missing)" })).toBeDisabled();
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith(saved);
  await userEvent.click(screen.getByRole("button", { name: "Remove VPN route 1" }));
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith(service);
});

it("canceled confirmations never change a VPN and refresh explicitly lists profiles", async () => {
  vi.mocked(confirmAction).mockResolvedValue(false);
  const refresh = vi.fn();
  render(<VpnSettings profiles={[{ ...profile, state: "disconnected" }]} projects={[]} onRefresh={refresh} />);
  await userEvent.click(screen.getByRole("button", { name: "Connect" }));
  await userEvent.click(screen.getByRole("button", { name: "Remove" }));
  expect(api.connectVpn).not.toHaveBeenCalled();
  expect(api.removeVpnProfile).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "Refresh profiles" }));
  await waitFor(() => expect(refresh).toHaveBeenCalled());
  expect(api.listVpnProfiles).toHaveBeenCalled();
});
it("validates optional corporate IPv4 settings and imports a private TCP target", async () => {
  render(<VpnSettings profiles={[]} projects={[]} onRefresh={vi.fn()} />);
  await fillImport();
  const dns = screen.getByLabelText(/Corporate DNS IPv4/);
  const icmp = screen.getByLabelText(/ICMP health target IPv4/);
  const health = screen.getByLabelText(/TCP health endpoint/);
  await userEvent.type(dns, "10.0.0.999");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  expect(screen.getByRole("alert")).toHaveTextContent("IPv4");
  expect(api.importVpnProfile).not.toHaveBeenCalled();
  await userEvent.clear(dns); await userEvent.type(dns, "10.0.0.53");
  await userEvent.type(icmp, "10.0.0.54");
  await userEvent.clear(health); await userEvent.type(health, "database.private.invalid:5432");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  await waitFor(() => expect(api.importVpnProfile).toHaveBeenCalledWith(expect.objectContaining({ dnsAddress: "10.0.0.53", healthIcmpTarget: "10.0.0.54", healthTarget: "database.private.invalid:5432" })));
  expect(dns).toHaveValue(""); expect(icmp).toHaveValue("");
});
it("allows disconnecting an unhealthy gateway without deleting its profile", async () => {
  render(<VpnSettings profiles={[{ ...profile, state: "failed", error: "Gateway health failed" }]} projects={[]} onRefresh={vi.fn()} />);
  const disconnect = screen.getByRole("button", { name: "Disconnect" });
  expect(disconnect).toBeEnabled();
  await userEvent.click(disconnect);
  await waitFor(() => expect(api.disconnectVpn).toHaveBeenCalledWith(profile.id));
  expect(api.removeVpnProfile).not.toHaveBeenCalled();
});

it("imports certificate-only encrypted keys without account credentials and clears the passphrase", async () => {
  render(<VpnSettings profiles={[]} projects={[]} onRefresh={vi.fn()} />);
  await fillImport();
  await userEvent.selectOptions(screen.getByLabelText(/Authentication/), "certificate");
  expect(screen.queryByLabelText("VPN username")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("VPN password")).not.toBeInTheDocument();
  const passphrase = screen.getByLabelText(/Private key passphrase/);
  await userEvent.type(passphrase, "synthetic-key-passphrase");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  await waitFor(() => expect(api.importVpnProfile).toHaveBeenCalledWith(expect.objectContaining({ username: "", password: "", keyPassphrase: "synthetic-key-passphrase" })));
  expect(passphrase).toHaveValue("");
  expect(screen.getByRole("status")).not.toHaveTextContent("synthetic-key-passphrase");
});
it("preserves a key passphrase after certificate import failure without reflecting it", async () => {
  vi.mocked(api.importVpnProfile).mockRejectedValue(new Error("synthetic-key-passphrase"));
  render(<VpnSettings profiles={[]} projects={[]} onRefresh={vi.fn()} />);
  await userEvent.selectOptions(screen.getByLabelText(/Authentication/), "certificate");
  await userEvent.type(screen.getByLabelText("Profile name"), "Certificate fixture");
  await userEvent.click(screen.getByRole("button", { name: "Choose .ovpn file" }));
  await screen.findByText("fixture.ovpn");
  await userEvent.type(screen.getByLabelText(/Private key passphrase/), "synthetic-key-passphrase");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  expect(await screen.findByRole("alert")).not.toHaveTextContent("synthetic-key-passphrase");
  expect(screen.getByLabelText(/Private key passphrase/)).toHaveValue("synthetic-key-passphrase");
  expect(screen.getByLabelText("Profile name")).toHaveValue("Certificate fixture");
});
it("supports a separate encrypted-key passphrase with username/password authentication", async () => {
  render(<VpnSettings profiles={[]} projects={[]} onRefresh={vi.fn()} />);
  await fillImport();
  await userEvent.type(screen.getByLabelText(/Private key passphrase/), "synthetic-separate-passphrase");
  await userEvent.click(screen.getByRole("button", { name: "Import profile" }));
  await waitFor(() => expect(api.importVpnProfile).toHaveBeenCalledWith(expect.objectContaining({ username: "synthetic-user", password: "synthetic-password", keyPassphrase: "synthetic-separate-passphrase" })));
});

it("answers a profile challenge while Connect waits without affecting a healthy independent profile", async () => {
  let finishConnect!: (profile: VpnProfile) => void;
  vi.mocked(api.connectVpn).mockReturnValue(new Promise((resolve) => { finishConnect = resolve; }));
  const refresh = vi.fn().mockRejectedValue(new Error("Metadata refresh unavailable"));
  const other = { ...profile, id: "certificate-fixture", name: "Certificate fixture", state: "ready" };
  const view = render(<VpnSettings profiles={[{ ...profile, state: "disconnected" }, other]} projects={[]} onRefresh={refresh} />);
  const pendingProfile = within(screen.getByRole("article", { name: "Fixture VPN" }));
  await userEvent.click(pendingProfile.getByRole("button", { name: "Connect" }));
  await waitFor(() => expect(api.connectVpn).toHaveBeenCalledWith(profile.id));
  view.rerender(<VpnSettings profiles={[{ ...profile, state: "authRequired", authChallenge: { id: "challenge-a", prompt: "Enter the current PIN" } }, other]} projects={[]} onRefresh={refresh} />);
  const pin = pendingProfile.getByLabelText("PIN / challenge response");
  expect(pin).toHaveAttribute("type", "password");
  expect(pin).toBeEnabled();
  await userEvent.type(pin, "synthetic-current-pin");
  const send = pendingProfile.getByRole("button", { name: "Send response" });
  expect(send).toBeEnabled();
  await userEvent.click(send);
  await waitFor(() => expect(api.respondVpnChallenge).toHaveBeenCalledWith(profile.id, "challenge-a", "synthetic-current-pin"));
  expect(pin).toHaveValue("");
  expect(pendingProfile.getByRole("status")).toHaveTextContent("Response accepted");
  expect(pendingProfile.getByRole("button", { name: "Response sent" })).toBeDisabled();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(api.connectVpn).toHaveBeenCalledTimes(1);
  expect(api.disconnectVpn).not.toHaveBeenCalled();
  expect(api.importVpnProfile).not.toHaveBeenCalled();
  expect(within(screen.getByRole("article", { name: "Certificate fixture" })).queryByLabelText("PIN / challenge response")).not.toBeInTheDocument();
  await act(async () => finishConnect(profile));
});
it("discards an unsent response when the challenge changes or disappears", async () => {
  const challenged = { ...profile, state: "authRequired", authChallenge: { id: "challenge-a", prompt: "Enter a PIN" } };
  const view = render(<VpnSettings profiles={[challenged]} projects={[]} onRefresh={vi.fn()} />);
  await userEvent.type(screen.getByLabelText("PIN / challenge response"), "synthetic-old-pin");
  view.rerender(<VpnSettings profiles={[{ ...challenged, authChallenge: { id: "challenge-b", prompt: "Enter the next PIN" } }]} projects={[]} onRefresh={vi.fn()} />);
  expect(screen.getByLabelText("PIN / challenge response")).toHaveValue("");
  expect(screen.getByText("Enter the next PIN")).toBeVisible();
  await userEvent.type(screen.getByLabelText("PIN / challenge response"), "synthetic-new-pin");
  view.rerender(<VpnSettings profiles={[{ ...challenged, state: "ready", authChallenge: null }]} projects={[]} onRefresh={vi.fn()} />);
  expect(screen.queryByLabelText("PIN / challenge response")).not.toBeInTheDocument();
  view.rerender(<VpnSettings profiles={[challenged]} projects={[]} onRefresh={vi.fn()} />);
  expect(screen.getByLabelText("PIN / challenge response")).toHaveValue("");
  expect(api.respondVpnChallenge).not.toHaveBeenCalled();
});
it("clears rejected responses and never reflects private response errors", async () => {
  vi.mocked(api.respondVpnChallenge).mockRejectedValue(new Error("synthetic-rejected-pin"));
  const write = vi.spyOn(Storage.prototype, "setItem");
  render(<VpnSettings profiles={[{ ...profile, state: "authRequired", authChallenge: { id: "challenge-a", prompt: "Current PIN" } }]} projects={[]} onRefresh={vi.fn()} />);
  await userEvent.type(screen.getByLabelText("PIN / challenge response"), "synthetic-rejected-pin");
  await userEvent.click(screen.getByRole("button", { name: "Send response" }));
  expect(await screen.findByRole("alert")).not.toHaveTextContent("synthetic-rejected-pin");
  expect(screen.getByLabelText("PIN / challenge response")).toHaveValue("");
  expect(write).not.toHaveBeenCalled();
  write.mockRestore();
});

it("cancels pending authentication while Connect waits and leaves another healthy profile alone", async () => {
  let finishConnect!: (profile: VpnProfile) => void;
  vi.mocked(api.connectVpn).mockReturnValue(new Promise((resolve) => { finishConnect = resolve; }));
  const refresh = vi.fn().mockRejectedValue(new Error("Metadata refresh unavailable"));
  const other = { ...profile, id: "certificate-fixture", name: "Certificate fixture", state: "ready" };
  const view = render(<VpnSettings profiles={[{ ...profile, state: "disconnected" }, other]} projects={[]} onRefresh={refresh} />);
  const pending = within(screen.getByRole("article", { name: "Fixture VPN" }));
  await userEvent.click(pending.getByRole("button", { name: "Connect" }));
  await waitFor(() => expect(api.connectVpn).toHaveBeenCalledWith(profile.id));
  view.rerender(<VpnSettings profiles={[{ ...profile, state: "authRequired", authChallenge: { id: "challenge-a", prompt: "Enter PIN" } }, other]} projects={[]} onRefresh={refresh} />);
  await userEvent.type(pending.getByLabelText("PIN / challenge response"), "synthetic-unsent-pin");
  const cancel = pending.getByRole("button", { name: "Cancel authentication" });
  expect(cancel).toBeEnabled();
  await userEvent.click(cancel);
  await waitFor(() => expect(api.disconnectVpn).toHaveBeenCalledWith(profile.id));
  expect(api.disconnectVpn).toHaveBeenCalledTimes(1);
  expect(api.respondVpnChallenge).not.toHaveBeenCalled();
  expect(pending.getByLabelText("PIN / challenge response")).toHaveValue("");
  expect(pending.getByRole("status")).toHaveTextContent("Authentication cancelled");
  expect(pending.getByRole("button", { name: "Cancel authentication" })).toBeDisabled();
  expect(pending.getByRole("button", { name: "Send response" })).toBeDisabled();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(api.removeVpnProfile).not.toHaveBeenCalled();
  await act(async () => finishConnect(profile));
});
it("one profile’s pending PIN submission does not disable another profile’s challenge", async () => {
  let finishResponse!: () => void;
  vi.mocked(api.respondVpnChallenge).mockReturnValueOnce(new Promise((resolve) => { finishResponse = resolve; }));
  const first = { ...profile, state: "authRequired", authChallenge: { id: "challenge-a", prompt: "First PIN" } };
  const second = { ...first, id: "second-fixture", name: "Second fixture", authChallenge: { id: "challenge-b", prompt: "Second PIN" } };
  render(<VpnSettings profiles={[first, second]} projects={[]} onRefresh={vi.fn()} />);
  const firstPanel = within(screen.getByRole("article", { name: "Fixture VPN" }));
  const secondPanel = within(screen.getByRole("article", { name: "Second fixture" }));
  await userEvent.type(firstPanel.getByLabelText("PIN / challenge response"), "synthetic-first-pin");
  await userEvent.click(firstPanel.getByRole("button", { name: "Send response" }));
  expect(firstPanel.getByLabelText("PIN / challenge response")).toBeDisabled();
  expect(secondPanel.getByLabelText("PIN / challenge response")).toBeEnabled();
  expect(secondPanel.getByRole("button", { name: "Cancel authentication" })).toBeEnabled();
  await userEvent.type(secondPanel.getByLabelText("PIN / challenge response"), "synthetic-second-pin");
  expect(secondPanel.getByRole("button", { name: "Send response" })).toBeEnabled();
  await act(async () => finishResponse());
});

it("can connect a second independent profile while the first waits for its PIN", async () => {
  const finish: Record<string, (value: VpnProfile) => void> = {};
  vi.mocked(api.connectVpn).mockImplementation((id) => new Promise((resolve) => { finish[id] = resolve; }));
  const first = { ...profile, state: "disconnected" };
  const second = { ...first, id: "second-fixture", name: "Second fixture" };
  const refresh = vi.fn();
  const view = render(<VpnSettings profiles={[first, second]} projects={[]} onRefresh={refresh} />);
  await userEvent.click(within(screen.getByRole("article", { name: "Fixture VPN" })).getByRole("button", { name: "Connect" }));
  await waitFor(() => expect(api.connectVpn).toHaveBeenCalledWith(first.id));
  view.rerender(<VpnSettings profiles={[{ ...first, state: "authRequired", authChallenge: { id: "challenge-a", prompt: "First PIN" } }, second]} projects={[]} onRefresh={refresh} />);
  const secondConnect = within(screen.getByRole("article", { name: "Second fixture" })).getByRole("button", { name: "Connect" });
  expect(secondConnect).toBeEnabled();
  await userEvent.click(secondConnect);
  await waitFor(() => expect(api.connectVpn).toHaveBeenCalledWith(second.id));
  expect(api.connectVpn).toHaveBeenCalledTimes(2);
  expect(screen.getByLabelText("PIN / challenge response")).toBeEnabled();
  expect(api.disconnectVpn).not.toHaveBeenCalled();
  await act(async () => { finish[first.id](profile); finish[second.id]({ ...second, state: "ready" }); });
});
