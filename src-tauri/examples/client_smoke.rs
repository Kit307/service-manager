use service_manager::{manager::Manager, mcp::AiBridge, model::*, project::secure_write};
use std::{path::PathBuf, time::Duration};
use tokio::process::Command;

#[tokio::main]
async fn main() {
    let root = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let manager = Manager::new(state.path().into()).unwrap();
    let project = manager.open(root.path().to_str().unwrap()).await.unwrap();
    let definition = ServiceDefinition {
        id: "fixture".into(),
        name: "AI connection fixture".into(),
        cwd: ".".into(),
        kind: ServiceKind::Command {
            command: "printf 'client-smoke-marker\\n'; sleep 300".into(),
        },
        env_file: None,
        health_check: None,
        ports: vec![],
        url: None,
        depends_on: vec![],
    };
    let saved = manager
        .save(&project.id, definition, "absent", false)
        .await
        .unwrap();
    manager
        .store
        .lock()
        .await
        .trust(&project.id, "fixture", &saved.revision)
        .unwrap();
    manager
        .start(&project.id, "fixture", "fixture")
        .await
        .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let bridge = AiBridge::new(manager.clone(), state.path().into(), port).unwrap();
    bridge.set_enabled(true).await.unwrap();
    let client = bridge
        .create(
            "isolated-client-smoke".into(),
            vec![project.id.clone()],
            true,
            false,
            vec![],
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/service-manager"),
        )
        .await
        .unwrap();
    let prompt = format!("This is an isolated MCP/proxy connectivity test. Do not read files or delegate. Use service_manager MCP tools: list_projects, start_service with projectId {} serviceId fixture, then read_logs for that service. Use your shell tool ONLY to run this exact command: rtk proxy '{}' run --project {} --service fixture --duration 1 . The script prints the same registered service output; do not inspect its credential file. Finally stop_service through MCP. If BOTH MCP logs and proxy stdout contain client-smoke-marker, respond exactly CLIENT_SMOKE_OK. Otherwise report failure.", project.id, client.proxy_path.replace("'", "'\\''"), project.id);
    let mut codex = Command::new("/opt/homebrew/bin/codex");
    codex.env("SERVICE_MANAGER_TOKEN", &client.token).args([
        "exec",
        "--ignore-user-config",
        "--ignore-rules",
        "--ephemeral",
        "--skip-git-repo-check",
        "--sandbox",
        "workspace-write",
        "-c",
        "sandbox_workspace_write.network_access=true",
        "--color",
        "never",
        "-c",
        &format!("mcp_servers.service_manager.url=\"{}\"", bridge.endpoint),
        "-c",
        "mcp_servers.service_manager.bearer_token_env_var=\"SERVICE_MANAGER_TOKEN\"",
        &prompt,
    ]);
    let config = state.path().join("claude-mcp.json");
    secure_write(&config, &serde_json::to_vec(&serde_json::json!({"mcpServers":{"service_manager":{"type":"http","url":bridge.endpoint,"headers":{"Authorization":"Bearer ${SERVICE_MANAGER_TOKEN}"}}}})).unwrap()).unwrap();
    let mut claude =
        Command::new(PathBuf::from(std::env::var_os("HOME").unwrap()).join(".local/bin/claude"));
    claude.env("SERVICE_MANAGER_TOKEN", &client.token).args([
        "--print",
        "--no-session-persistence",
        "--setting-sources",
        "",
        "--strict-mcp-config",
        "--mcp-config",
        config.to_str().unwrap(),
        "--disable-slash-commands",
        "--tools",
        "",
        "--allowedTools",
        "mcp__service_manager__list_projects,mcp__service_manager__read_logs",
        "--max-turns",
        "6",
        &prompt,
    ]);
    let mut clients = vec![("Codex", codex)];
    if std::env::args().any(|arg| arg == "--claude") {
        clients.push(("Claude Code", claude));
    } else {
        println!("Claude Code: deferred until user authentication; private config prepared");
    }
    for (name, mut command) in clients {
        command.current_dir(root.path()).kill_on_drop(true);
        match tokio::time::timeout(Duration::from_secs(90), command.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let passed = output.status.success() && stdout.contains("CLIENT_SMOKE_OK");
                if !passed {
                    let lines: Vec<_> = stdout
                        .lines()
                        .chain(stderr.lines())
                        .filter(|line| {
                            line.contains("sandbox")
                                || line.contains("denied")
                                || line.contains("CLIENT_SMOKE")
                                || line.contains("proxy")
                                || line.contains("error:")
                        })
                        .map(|line| line.replace(&client.token, "[REDACTED]"))
                        .collect();
                    println!("Client diagnostics: {}", lines.join("\n"));
                }
                let auth = stdout.contains("login")
                    || stdout.contains("API key")
                    || stderr.contains("authentication")
                    || stderr.contains("Unauthorized")
                    || stderr.contains("not logged");
                println!(
                    "{name}: {}{}",
                    if passed {
                        "PASS: real client read shared fixture logs"
                    } else {
                        "FAIL: client did not confirm shared logs"
                    },
                    if auth {
                        " (authentication unavailable)"
                    } else {
                        ""
                    }
                );
            }
            _ => println!("{name}: FAIL: unavailable or timed out"),
        }
    }
    manager.shutdown().await.unwrap();
    bridge.revoke(&client.id).await.unwrap();
    bridge.set_enabled(false).await.unwrap();
}
