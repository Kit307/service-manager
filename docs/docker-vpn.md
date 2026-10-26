# Shared Docker VPN

Docker VPN is opt-in for command services. Import a profile on the central **Docker VPN** page, then select its profile and a Linux runtime image in a service’s Settings. Native services and user-managed Compose files keep their existing execution path. Switching a command service to Compose removes its incompatible VPN selection.

## Prepare and import

1. Start your local Docker engine. The manager supports local Docker, not a remote daemon.
2. Choose **Download gateway image** to prepare `qmcgaw/gluetun:v3.41.0`. Downloads are explicit; Connect and Run never pull a missing image automatically.
3. Name the VPN and select one `.ovpn` file through the native picker. The configuration must contain inline certificates/keys. Choose Username/password when it contains `auth-user-pass`, or Certificate/key only for certificate authentication without account credentials. Enter the separate Private key passphrase when the inline key is encrypted; it works with either authentication method. Browser SSO is unsupported. Version 0.8.1 adds certificate-only authentication and encrypted private-key passphrases; version 0.8.2 adds a separate dynamic OpenVPN PIN challenge flow after account authentication.
4. Enter credentials privately in the import form. The app copies the configuration, credentials and any private key passphrase into private application storage; project files and public profile metadata contain neither. Successful import clears the private form fields. The app never displays raw configuration or gateway logs.
5. Set a TCP health endpoint as `host:port`, default `cloudflare.com:443`. Port 443 uses TLS; other ports check TCP connectivity, so a reachable private database endpoint on port 5432 is supported. When Internet access is blocked, use an endpoint reachable through the tunnel. Set optional Corporate DNS IPv4 for private hostname resolution; blank keeps Gluetun’s default. Set optional ICMP health target IPv4 to a reachable ping target, for example the private DNS IP if it responds; blank uses the VPN server (`0.0.0.0`). Both TCP and ICMP health paths must work for the configured network. Health status is an observation, not independent proof of traffic isolation.
6. If services need inbound local access, configure shared mappings as `HOST:CONTAINER`, comma-separated. Listeners bind only to `127.0.0.1`. Host and container ports must each be unique, with at most 32 mappings. Container ports 53, 8000, 9999 and 11940 are reserved for VPN DNS, control, health and authentication. Configure mappings before connecting.

The profile list shows state, a bounded public error, published ports and active service references. States include `disconnected`, `connecting`, `ready`, `failed` and `unavailable`. Connect, Disconnect and Remove require an explicit confirmation. Active service users disable Disconnect and Remove; the backend independently enforces ownership and active-user checks. Stop those services first. Docker ownership changes or an unavailable daemon require investigation; do not substitute `docker rm`, `down` or `prune`.

## Configure a service

Select a VPN profile in a **Shell command** service’s Network settings. Choose a Linux runtime image with the required runtime, such as `node:22-bookworm-slim` or `golang:1.24-bookworm`, then request **Download runtime image**. Selecting a profile does not rewrite the saved run command or download an image.

Edit the command for Linux before running. The working folder is mounted into the runtime container; Mac-specific absolute paths, binaries and login-shell setup do not work there. Bind the service to `0.0.0.0` when using a central published mapping. The service’s Ports field declares host listeners for diagnostics; it does not publish a worker container port. Dependencies, trust/revision checks, logs, history and lifecycle controls continue to use the existing service identity.

The optional project field is:

```json
"vpn": {"profileId": "DISCOVERED_PROFILE_ID", "image": "node:22-bookworm-slim"}
```

Only command services accept this field. Omit it entirely for native execution. A missing saved profile remains selected as unavailable until the user explicitly changes the network; it is never silently converted to native execution.

Several services can select the same profile and share its gateway. Independent profiles can connect at the same time. The tunnel belongs to Docker; the Mac itself does not join it. Stopping one service releases only its use of that profile. A VPN-selected service must not fall back to a host command when the tunnel cannot start or stay healthy.

## AI discovery and privacy

MCP exposes only `list_vpn_profiles {}` → `{profiles: [...]}` for VPN discovery. It returns public `id`, `name`, `publishedPorts`, `healthTarget`, `dnsAddress`, `healthIcmpTarget`, `state`, `error` and `users: [{projectId, serviceId}]`. Every request needs a valid, unrevoked workspace token. Credential import, tunnel lifecycle, removal and image downloads remain desktop actions; no matching MCP mutation tools or new proxy commands exist.

A client can select a discovered profile in `upsert_service` using the existing revision-checked contract. This does not authorize starting unrelated services or importing credentials. Never request or copy raw `.ovpn` contents, credentials, private app files or gateway logs into diagnostics.

Private app storage protects against accidental project-file disclosure. Docker administrators can inspect container mounts and private material; this feature is not a sandbox for hostile code. Automated synthetic tests do not establish organization VPN compatibility or packet/DNS isolation. Those require separately authorized runtime checks against the intended environment.

Verification: production Manager methods were tested against a self-hosted synthetic OpenVPN server with two gateways and three workers. Shared use, repeated Run, Stop, post-crash disconnect protection and verified worker cleanup after one gateway stops all passed. Organization authentication, corporate DNS and blackholed-tunnel detection timing remain untested. See [runtime evidence](../.agents/evidence/TASK-0018/vpn-runtime-proof.md).

With corporate DNS configured, the gateway disables its built-in DNS server (`DNS_SERVER=off`) and workers use that private IPv4 resolver directly. The gateway capability set was exercised against the pinned runtime and is listed below. Workers receive no added capabilities. Neither setting connects the Mac itself to the VPN. DNS addresses and health targets are public network configuration, not credentials.

## Docker binding and recovery

The first Docker use binds VPN storage to the local socket and daemon identity. Switching Docker contexts or replacing that daemon makes profiles unavailable and refuses lifecycle or deletion operations. Switch back to the original daemon before disconnecting or removing profiles; the app does not delete private files while an original gateway might still use them.

After an app crash, the displayed service-user list contains only leases acquired by the current app session. Disconnect and Remove additionally scan real running Docker namespace users and refuse them even when the displayed list is empty. Stop surviving workers in their original environment before retrying. Do not force-delete the gateway or private profile files.

The gateway needs `NET_ADMIN`, `NET_RAW`, `CHOWN`, `DAC_OVERRIDE`, `SETUID` and `SETGID` for tunnel/firewall setup, ICMP, OpenVPN private-file initialization and dropping the OpenVPN user after handshake. These capabilities are confined to the gateway inside the Docker VM; workers drop all capabilities. Gluetun permits traffic within its attached Docker bridge as part of its normal firewall policy. Each profile gets an owned bridge; this is VPN routing, not isolation from Docker administrators or the local Docker network.

Services sharing a profile also share one container network namespace. Their listening ports must be distinct; two workers cannot both bind the same container port. Services on different profiles can use the same container port with different loopback host mappings. Linux dependencies and environment endpoints must be compatible with the selected container image.

Before creating a gateway, the app resolves remote VPN hostnames to IPv4 through the Mac’s normal DNS, bounds the lookup, and writes a private runtime copy. The imported original remains unchanged. IPv6-only remotes are unsupported. Existing gateway containers retain their resolved endpoint; if the provider changes its address, stop workers and remove/reimport the profile before reconnecting. This endpoint bootstrap does not connect the Mac to the tunnel.

A fixed PIN requested after the account password is still an interactive MFA challenge. In 0.8.1, username/password plus private-key passphrase support does not implement that second-stage challenge. AUTH_FAILED alone must not be treated as proof of a wrong password. Certificate-only authentication does not relax the original server-certificate checks.

## Dynamic PIN authentication (0.8.2)

After Connect submits the saved username/password, a server can request a dynamic OpenVPN CRV1 challenge. The profile enters `authRequired` and shows its public prompt plus a private PIN/challenge-response field. Enter the current response for that profile and choose **Send response**. Both Send response and Cancel authentication remain available while Connect waits for authentication. Cancel authentication disconnects only that profile and discards its unsent PIN; the PIN is sent separately, never concatenated with the saved account password.

A response stays only in memory while this challenge is active. The UI clears it after submission, a failed submission, a changed challenge ID or leaving the challenge view. It is not saved into profile metadata, project configuration, logs or history. After an accepted response, a metadata-refresh failure does not invite sending the same response again. Wait for the profile’s updated state. A new challenge requires its own current response. Browser SSO remains unsupported.

Each profile keeps its own gateway and authentication challenge. Answering a challenge for one profile does not reconnect or replace an already healthy certificate-only profile. MCP exposes no tool for submitting PINs; response entry is a desktop action.
