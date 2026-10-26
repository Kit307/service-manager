import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { api, pickVpnConfig } from "../lib/api";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), ask: vi.fn() }));
afterEach(() => { vi.clearAllMocks(); vi.unstubAllGlobals(); });
it("uses native VPN command names and public payload contracts", async () => {
  const input = { name: "Fixture", configPath: "/private/fixture.ovpn", username: "synthetic", password: "synthetic", keyPassphrase: "synthetic-key", publishedPorts: [], healthTarget: "database.private.invalid:5432", dnsAddress: "10.0.0.53", healthIcmpTarget: "10.0.0.54" };
  await api.listVpnProfiles();
  await api.importVpnProfile(input);
  await api.connectVpn("fixture");
  await api.disconnectVpn("fixture");
  await api.removeVpnProfile("fixture");
  await api.pullContainerImage("qmcgaw/gluetun:v3.41.0");
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["list_vpn_profiles", {}], ["import_vpn_profile", { input }], ["connect_vpn", { id: "fixture" }],
    ["disconnect_vpn", { id: "fixture" }], ["remove_vpn_profile", { id: "fixture" }],
    ["pull_container_image", { image: "qmcgaw/gluetun:v3.41.0" }],
  ]);
});
it("opens a single native ovpn file picker without reading config into the UI", async () => {
  vi.stubGlobal("__TAURI_INTERNALS__", {});
  await pickVpnConfig();
  expect(open).toHaveBeenCalledWith({ multiple: false, directory: false, filters: [{ name: "OpenVPN", extensions: ["ovpn"] }] });
});
