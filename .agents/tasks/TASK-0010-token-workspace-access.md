# TASK-0010 - Token-wide workspace access

## Objective
One valid MCP/proxy token can discover, read, run, stop, restart and edit every registered service, including folders added later.

## Approved scope
The user explicitly selected full read/control/edit access on 2026-10-20. Existing tokens adopt the same workspace-wide policy. Remove project allowlists and permission checkboxes from the UI. Keep bearer authentication, revocation, disabled connection and Origin rejection. Project IDs remain target identities, not grants. Keep allowed parent roots for filesystem create/open operations and all configuration/path/revision/process-ownership protections. A token authorizes current registered commands on MCP start/restart; revalidation still rejects replacement after trust.

## Implementation and verification
- Reproduce current denial with an empty-scope token and a folder registered later.
- Remove scoped authorization in the shared MCP path used by native MCP and proxy; keep legacy serialized fields/API arguments compatible.
- Simplify profile creation and show full-workspace execution access explicitly.
- Update contract/docs and focused real-SDK tests including revocation, invalid token, disabled connection, late registration, editing, command trust, stale revision and root containment.
- Run frontend checks and native tests/Clippy; prepare v0.5.0 artifact after UI task.

## Non-goals
Unrestricted filesystem access, token removal, remote/LAN service, deployment, personal credential rewriting or restarting installed services.

## Completion
Implemented full-workspace access for existing/new valid tokens, simplified UI and preserved auth/revocation/root/revision/ownership checks. Real SDK/proxy and legacy-token tests passed; skill validation and frontend checks passed. Release artifacts are recorded with TASK-0011.
