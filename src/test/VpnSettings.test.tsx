import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { api, confirmAction, pickVpnConfig } from "../lib/api";
import { VpnSettings } from "../components/VpnSettings";
import { ServiceEditor } from "../components/ServiceEditor";
import type { ServiceDefinition, VpnProfile } from "../lib/types";

vi.mock("../lib/api", async (original) => ({
  ...await original<typeof import("../lib/api")>(),
  confirmAction: vi.fn(), pickVpnConfig: vi.fn(),
  api: { importVpnProfile: vi.fn(), connectVpn: vi.fn(), disconnectVpn: vi.fn(), removeVpnProfile: vi.fn(), listVpnProfiles: vi.fn(), pullContainerImage: vi.fn() },
}));
const profile: VpnProfile = { id: "fixture-vpn", name: "Fixture VPN", healthTarget: "cloudflare.com:443", publishedPorts: [{ host: 3000, container: 8080 }], state: "ready", users: [] };
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
it("preserves absent vpn for native and Compose services", async () => {
  const save = vi.fn();
  render(<ServiceEditor service={service} saving={false} onSave={save} onCancel={vi.fn()} />);
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith(service);
  await userEvent.click(screen.getByLabelText("Docker Compose"));
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith({ id: "fixture", name: "Fixture", type: "compose", file: "compose.yaml", cwd: "." });
});
it("saves chosen VPN/runtime image without changing command and removes vpn when switching to Compose", async () => {
  const save = vi.fn();
  render(<ServiceEditor service={service} vpnProfiles={[profile]} saving={false} onSave={save} onCancel={vi.fn()} />);
  await userEvent.selectOptions(screen.getByLabelText("VPN profile"), profile.id);
  const image = screen.getByLabelText("Linux runtime image");
  expect(image).toHaveValue("node:22-bookworm-slim");
  await userEvent.clear(image); await userEvent.type(image, "golang:1.24-bookworm");
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith({ ...service, vpn: { profileId: profile.id, image: "golang:1.24-bookworm" } });
  expect(api.pullContainerImage).not.toHaveBeenCalled();
  await userEvent.click(screen.getByLabelText("Docker Compose"));
  expect(screen.queryByLabelText("VPN profile")).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save.mock.lastCall?.[0]).not.toHaveProperty("vpn");
});
it("preserves missing saved profile until the user selects another network", async () => {
  const save = vi.fn();
  const saved: ServiceDefinition = { ...service, vpn: { profileId: "missing", image: "node:22-bookworm-slim" } };
  render(<ServiceEditor service={saved} vpnProfiles={[]} saving={false} onSave={save} onCancel={vi.fn()} />);
  expect(screen.getByRole("option", { name: "Unavailable profile (missing)" })).toBeDisabled();
  await userEvent.click(screen.getByRole("button", { name: "Save service" }));
  expect(save).toHaveBeenLastCalledWith(saved);
  await userEvent.selectOptions(screen.getByLabelText("VPN profile"), "");
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
it("downloads a chosen runtime explicitly and reports completion", async () => {
  render(<ServiceEditor service={{ ...service, vpn: { profileId: profile.id, image: "node:22-bookworm-slim" } }} vpnProfiles={[profile]} saving={false} onSave={vi.fn()} onCancel={vi.fn()} />);
  await userEvent.click(screen.getByRole("button", { name: "Download runtime image" }));
  expect(api.pullContainerImage).toHaveBeenCalledWith("node:22-bookworm-slim");
  expect(await screen.findByRole("status")).toHaveTextContent("Runtime image is ready locally");
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
