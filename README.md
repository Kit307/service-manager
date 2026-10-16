# Service Manager

A desktop app for running project services without typing commands in a terminal.

Status: project initialized; design recorded. The application and MCP integration are not implemented yet.

## Agreed workflow

1. Open a project folder.
2. Load its saved service configuration, or add a service and its run command.
3. Run, stop, or restart services and view live logs.
4. Reopen the folder to restore its saved configuration.
5. Let Codex and Claude use MCP to manage registered services and read their logs.

Each project can contain multiple services with different working directories and commands, including Go, Node.js, and Docker Compose.

See [the design](docs/superpowers/specs/2026-10-16-service-manager-design.md) for behavior and implementation boundaries.
