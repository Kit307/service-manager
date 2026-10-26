# Synthetic Gluetun runtime proof

Date: 2026-10-26. Docker daemon 29.2.1; qmcgaw/gluetun:v3.41.0 digest sha256:6b54856716d0de56e5bb00a77029b0adea57284cf5a466f23aad5979257d3045.

No real VPN account/configuration or current services were used. The fixture creates a temporary inline CA and dummy username/password, uses the documentation-only unreachable endpoint 198.51.100.1, and deletes its named containers/network and temporary keys on exit.

## Findings

- NET_ADMIN alone fails startup: `chown /etc/openvpn/target.ovpn: operation not permitted`.
- Adding CHOWN reaches the next check but OpenVPN cannot read its owned mode-0600 auth.conf (`Permission denied`).
- NET_ADMIN + CHOWN + DAC_OVERRIDE with cap-drop ALL and no-new-privileges passes configuration/auth-file startup. A successful local tunnel subsequently revealed that the image runs OpenVPN as nonrootuser and also requires SETUID after handshake. The full positive proof uses NET_ADMIN, NET_RAW, CHOWN, DAC_OVERRIDE, SETUID and SETGID; the initial absent-tunnel check alone cannot establish the complete capability set.
- With tunnel absent: INPUT and OUTPUT firewall policies are DROP; shared-namespace worker HTTP to 1.1.1.1 and normal DNS lookup fail. Those public destinations were not positively baselined. A reachable local Docker-bridge HTTP peer remains reachable: Gluetun permits its attached subnet, so this is not isolation from Docker-bridge peers/host services.
- Worker sees the exact private resolver configuration and cannot overwrite the read-only /etc/resolv.conf mount.
- All named fixture resources were removed after each run.

The initial absent-tunnel probes alone establish initialization and read-only resolver behavior, with the attached-subnet exception above. Successful connection and app-level cleanup are verified by the subsequent Manager test below. No organization VPN or corporate DNS server was tested.

## Pinned-source configuration findings

- v3.41.0 `internal/healthcheck/checker.go`: port 443 uses TCP plus certificate-verified TLS; other explicit ports use plain TCP. A private database port is a valid TCP target.
- `internal/configuration/settings/health.go`: small checks accept only ICMP or DNS; empty ICMP targets revert to public defaults. ICMP 0.0.0.0 selects the VPN server IP.
- `internal/healthcheck/dns/dns.go`: fallback DNS health checks query github.com through hardcoded public resolvers. They do not use the selected private resolver.
- `internal/configuration/settings/dns.go`: DNS_UPSTREAM_RESOLVERS accepts named providers, not an arbitrary corporate resolver IP. A private DNS_ADDRESS with DNS_SERVER=off is the supported simple override; workers must use that address.

Sources: https://github.com/qdm12/gluetun/tree/v3.41.0/internal/healthcheck and https://github.com/qdm12/gluetun/tree/v3.41.0/internal/configuration/settings .

## Actual Manager integration proof

Command: `rtk proxy /Users/krit/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --test vpn_docker -- --ignored --nocapture`.

Result: PASS, 1 test, 37.30 seconds (including Python fixture orchestration). The test uses the production Manager, VpnManager, generated Compose worker, pinned Docker sessions and lifecycle methods.

Verified:

- Two profiles establish separate healthy Gluetun/OpenVPN gateways against a self-hosted synthetic server, with no public/host published server ports.
- Three managed command workers reach the synthetic VPN-only address 10.66.0.1:5432. Two workers share profile A; the third uses profile B. Exactly two owned gateways run.
- A repeated start returns the same runId. Logs contain the expected synthetic marker.
- Active users reject disconnect. A fresh VpnManager instance with no in-memory leases still rejects disconnect by discovering real Docker namespace users.
- Stopping one A worker leaves its sibling and B reachable.
- Stopping gateway A triggers Manager failure handling. A's remaining worker becomes Failed only after Docker confirms its container stopped. B remains active and reaches its VPN target.
- Manager shutdown stops B and its gateways. Exact fixture resources and temporary keys are cleaned.

The fixture needs preinstalled Gluetun/Alpine images, local Docker and OpenSSL. It generates temporary certificates and dummy credentials. It precreates correctly owned private bridges with synthetic server IPs solely to avoid publishing test server ports; production gateway adoption and labels are still verified.

Limitations: the induced failure is a stopped gateway, not a measured blackholed UDP tunnel. Real corporate authentication, DNS, packet-loss timing and ICMP policies remain untested. The fixture found that Gluetun custom provider accepts only IP-address remote endpoints. The source now normalizes hostname remotes to IPv4 in a private runtime copy before new gateway creation; bounded resolver/parser tests cover this policy.
