use crate::model::{AppError, Result};
use rmcp::{
    model::CallToolRequestParams,
    service::RunningService,
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    RoleClient, ServiceExt,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{io::Write, path::PathBuf, time::Duration};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    pub endpoint: String,
    pub token: String,
}
pub type McpClient = RunningService<RoleClient, ()>;
pub async fn connect(credentials: Credentials) -> Result<McpClient> {
    let url = reqwest::Url::parse(&credentials.endpoint)
        .map_err(|_| AppError::new("CONFIG_INVALID", "Invalid proxy endpoint."))?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.path() != "/mcp"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
    {
        return Err(AppError::new(
            "CONFIG_INVALID",
            "Proxy endpoints must be authenticated local loopback MCP addresses.",
        ));
    }
    let http = reqwest_mcp::Client::builder()
        .no_proxy()
        .redirect(reqwest_mcp::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| {
            AppError::new(
                "MCP_UNAVAILABLE",
                "Could not initialize the local HTTP client.",
            )
        })?;
    let mut config = StreamableHttpClientTransportConfig::with_uri(credentials.endpoint)
        .auth_header(credentials.token);
    config.reinit_on_expired_session = false;
    let transport = StreamableHttpClientTransport::with_client(http, config);
    tokio::time::timeout(Duration::from_secs(5), ().serve(transport)).await.map_err(|_| AppError::new("MCP_UNAVAILABLE", "MCP connection timed out. Open the desktop app and enable AI Connections."))?.map_err(|_| AppError::new("MCP_UNAVAILABLE", "Cannot connect. Open the desktop app, enable AI Connections, and check this profile has not been revoked."))
}
pub async fn call(client: &McpClient, name: &str, args: Value) -> Result<Value> {
    let result = client
        .call_tool(
            CallToolRequestParams::new(name.to_string())
                .with_arguments(args.as_object().cloned().unwrap_or_default()),
        )
        .await
        .map_err(|_| {
            AppError::new(
                "MCP_UNAVAILABLE",
                "The MCP request failed. Query service status before repeating a lifecycle action.",
            )
        })?;
    let data = result.structured_content.unwrap_or(Value::Null);
    if result.is_error == Some(true) {
        return Err(AppError::new(
            data.get("code")
                .and_then(Value::as_str)
                .unwrap_or("MCP_ERROR"),
            data.get("message")
                .and_then(Value::as_str)
                .unwrap_or("The tool request failed."),
        ));
    }
    Ok(data)
}
struct Options {
    profile: PathBuf,
    action: String,
    project: String,
    service: String,
    cursor: Option<String>,
    run: Option<String>,
    follow: bool,
    duration: u64,
}
fn parse(args: Vec<String>) -> Result<Options> {
    let fail = || {
        AppError::new("CONFIG_INVALID", "Usage: proxy --profile <private-file> list | run --project <id> --service <id> [--duration 1..60] | logs --project <id> --service <id> [--cursor N] [--run <id>] [--follow]")
    };
    let mut args = args.into_iter();
    if args.next().as_deref() != Some("--profile") {
        return Err(fail());
    }
    let profile = args.next().map(PathBuf::from).ok_or_else(fail)?;
    let action = args.next().ok_or_else(fail)?;
    if !["list", "run", "logs"].contains(&action.as_str()) {
        return Err(fail());
    }
    let mut result = Options {
        profile,
        follow: action == "run",
        action,
        project: String::new(),
        service: String::new(),
        cursor: None,
        run: None,
        duration: 15,
    };
    while let Some(key) = args.next() {
        if key == "--follow" {
            result.follow = true;
            continue;
        }
        let value = args.next().ok_or_else(fail)?;
        match key.as_str() {
            "--project" => result.project = value,
            "--service" => result.service = value,
            "--cursor" => {
                value.parse::<u64>().map_err(|_| fail())?;
                result.cursor = Some(value);
            }
            "--run" => result.run = Some(value),
            "--duration" => {
                result.duration = value.parse().map_err(|_| fail())?;
                if !(1..=60).contains(&result.duration) {
                    return Err(fail());
                }
            }
            _ => return Err(fail()),
        }
    }
    if result.action != "list" && (result.project.is_empty() || result.service.is_empty()) {
        return Err(fail());
    }
    Ok(result)
}
pub async fn run(args: Vec<String>) -> i32 {
    let result = async {
        let mut options = parse(args)?;
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::symlink_metadata(&options.profile)?;
        if metadata.file_type().is_symlink() || metadata.permissions().mode() & 0o077 != 0 || metadata.len() > 16384 { return Err(AppError::new("PERMISSION_DENIED", "Proxy credential file must be private, small, and not a symbolic link.")); }
        let credentials: Credentials = serde_json::from_slice(&std::fs::read(&options.profile)?).map_err(|_| AppError::new("CONFIG_INVALID", "Invalid proxy credential file."))?;
        let client = connect(credentials).await?;
        if options.action == "list" { let value = call(&client, "list_projects", json!({})).await?; writeln!(std::io::stdout().lock(), "{}", value)?; let _ = client.cancel().await; return Ok(0); }
        let key = json!({"projectId":options.project,"serviceId":options.service});
        if options.action == "run" { let snapshot = call(&client, "start_service", key.clone()).await?; options.run = snapshot.get("runId").and_then(Value::as_str).map(String::from); }
        let pump = async {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(options.duration); let mut bytes = 0; let mut cursor = options.cursor; let mut last_state = call(&client, "get_service_status", key.clone()).await?;
            let reason = loop {
                let mut query = key.clone(); if let Some(c) = &cursor { query["cursor"] = json!(c); }
                let page = call(&client, "read_logs", query).await?;
                if page.get("truncated").and_then(Value::as_bool) == Some(true) { eprintln!("[proxy] Earlier output was evicted. Showing available history."); }
                let records = page.get("records").and_then(Value::as_array).cloned().unwrap_or_default();
                let mut capped = false;
                for record in records {
                    if options.run.as_ref().is_some_and(|r| record.get("runId").and_then(Value::as_str) != Some(r)) { cursor = record.get("seq").and_then(Value::as_u64).map(|n| n.to_string()); continue; }
                    let line = format!("[{} {}] {}\n", record["timestamp"].as_str().unwrap_or(""), record["stream"].as_str().unwrap_or(""), record["text"].as_str().unwrap_or(""));
                    if bytes + line.len() > 128 * 1024 { capped = true; break; }
                    if std::io::stdout().lock().write_all(line.as_bytes()).is_err() { return Ok::<i32, AppError>(0); }
                    bytes += line.len(); cursor = record.get("seq").and_then(Value::as_u64).map(|n| n.to_string());
                }
                if capped { break "byteLimit"; }
                cursor = page.get("nextCursor").and_then(Value::as_str).map(String::from).or(cursor);
                last_state = call(&client, "get_service_status", key.clone()).await?;
                if options.run.as_ref().is_some_and(|r| last_state.get("runId").and_then(Value::as_str) != Some(r)) { break "runChanged"; }
                if !options.follow { break "snapshot"; }
                if last_state.get("state").and_then(Value::as_str).is_some_and(|s| s == "stopped" || s == "failed") { break "processExit"; }
                if tokio::time::Instant::now() >= deadline { break "durationLimit"; }
                tokio::time::sleep(Duration::from_millis(250)).await;
            };
            eprintln!("{}", json!({"projectId":options.project,"serviceId":options.service,"runId":options.run,"state":last_state.get("state"),"reason":reason,"nextCursor":cursor,"bytes":bytes,"serviceContinues":last_state.get("state").and_then(Value::as_str).is_some_and(|s| ["running","ready","starting"].contains(&s))}));
            Ok(if last_state.get("state").and_then(Value::as_str) == Some("failed") { 1 } else { 0 })
        };
        let code = tokio::select! { result = pump => result?, _ = tokio::signal::ctrl_c() => { eprintln!("[proxy] Detached. The managed service was not stopped."); 130 } };
        let _ = client.cancel().await; Ok::<i32, AppError>(code)
    }.await;
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!(
                "{}",
                serde_json::to_string(&e).unwrap_or_else(|_| "Proxy failed".into())
            );
            match e.code.as_str() {
                "CONFIG_INVALID" => 2,
                "PERMISSION_DENIED" | "PROJECT_NOT_AUTHORIZED" => 4,
                "MCP_UNAVAILABLE" | "IO_ERROR" => 3,
                _ => 1,
            }
        }
    }
}
