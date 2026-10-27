use crate::model::{AppError, Result, VpnRoute};
use std::net::{Ipv4Addr, SocketAddr};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

pub struct RelayTarget {
    pub route: VpnRoute,
    pub proxy: SocketAddr,
}

pub struct VpnRelaySet {
    tasks: Vec<JoinHandle<()>>,
}

fn error(message: &str) -> AppError {
    AppError::new("VPN_ERROR", message)
}

impl VpnRelaySet {
    pub async fn start(targets: Vec<RelayTarget>) -> Result<Self> {
        let mut listeners = Vec::with_capacity(targets.len());
        for target in targets {
            if !target.proxy.ip().is_loopback() {
                return Err(error("VPN proxy endpoints must stay on loopback."));
            }
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, target.route.local_port))
                .await
                .map_err(|_| {
                    error(&format!(
                        "VPN relay port {} is unavailable.",
                        target.route.local_port
                    ))
                })?;
            listeners.push((listener, target));
        }
        let tasks = listeners
            .into_iter()
            .map(|(listener, target)| {
                tokio::spawn(async move {
                    let mut connections = JoinSet::new();
                    loop {
                        tokio::select! {
                            accepted = listener.accept() => {
                                let Ok((client, _)) = accepted else { break };
                                let proxy = target.proxy;
                                let host = target.route.remote_host.clone();
                                let port = target.route.remote_port;
                                connections.spawn(async move {
                                    let _ = forward(client, proxy, &host, port).await;
                                });
                            }
                            Some(_) = connections.join_next(), if !connections.is_empty() => {}
                        }
                    }
                })
            })
            .collect();
        Ok(Self { tasks })
    }
}

impl Drop for VpnRelaySet {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

async fn forward(
    mut client: TcpStream,
    proxy: SocketAddr,
    remote_host: &str,
    remote_port: u16,
) -> Result<()> {
    let mut upstream = TcpStream::connect(proxy)
        .await
        .map_err(|_| error("VPN proxy is unavailable."))?;
    let destination = format!("{remote_host}:{remote_port}");
    upstream
        .write_all(
            format!("CONNECT {destination} HTTP/1.1\r\nHost: {destination}\r\n\r\n").as_bytes(),
        )
        .await
        .map_err(|_| error("VPN proxy request failed."))?;
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    while response.len() < 8192 {
        upstream
            .read_exact(&mut byte)
            .await
            .map_err(|_| error("VPN proxy closed before accepting the destination."))?;
        response.push(byte[0]);
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    if !response.ends_with(b"\r\n\r\n") {
        return Err(error("VPN proxy response was too large."));
    }
    let first_line = response
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    if !first_line.starts_with(b"HTTP/1.1 200 ") && !first_line.starts_with(b"HTTP/1.0 200 ") {
        return Err(error("VPN proxy rejected the destination."));
    }
    tokio::io::copy_bidirectional(&mut client, &mut upstream)
        .await
        .map_err(|_| error("VPN relay connection ended unexpectedly."))?;
    Ok(())
}
