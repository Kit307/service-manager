use crate::model::{AppError, Result, VpnRoute};
use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

pub struct RelayTarget {
    pub route: VpnRoute,
    pub proxy: SocketAddr,
}

pub struct VpnRelaySet {
    tasks: Vec<JoinHandle<()>>,
    traffic: Vec<RelayTraffic>,
}

#[derive(Default)]
struct Counters {
    tx: AtomicU64,
    rx: AtomicU64,
    active: AtomicU64,
}

#[derive(Clone)]
pub struct RelayTraffic {
    route: VpnRoute,
    counters: Arc<Counters>,
}

pub struct RelayTrafficSnapshot {
    pub route: VpnRoute,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub active_connections: u64,
}

impl RelayTraffic {
    pub fn snapshot(&self) -> RelayTrafficSnapshot {
        RelayTrafficSnapshot {
            route: self.route.clone(),
            tx_bytes: self.counters.tx.load(Ordering::Relaxed),
            rx_bytes: self.counters.rx.load(Ordering::Relaxed),
            active_connections: self.counters.active.load(Ordering::Relaxed),
        }
    }
}

struct Established(Arc<Counters>);

impl Drop for Established {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Relaxed);
    }
}

struct CountedStream {
    stream: TcpStream,
    counters: Arc<Counters>,
    tx: bool,
}

impl AsyncRead for CountedStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for CountedStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.stream).poll_write(cx, buf);
        if let Poll::Ready(Ok(bytes)) = result {
            let counter = if self.tx {
                &self.counters.tx
            } else {
                &self.counters.rx
            };
            counter.fetch_add(bytes as u64, Ordering::Relaxed);
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
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
        let traffic: Vec<_> = listeners
            .iter()
            .map(|(_, target)| RelayTraffic {
                route: target.route.clone(),
                counters: Arc::new(Counters::default()),
            })
            .collect();
        let tasks = listeners
            .into_iter()
            .zip(traffic.iter().cloned())
            .map(|((listener, target), traffic)| {
                tokio::spawn(async move {
                    let mut connections = JoinSet::new();
                    loop {
                        tokio::select! {
                            accepted = listener.accept() => {
                                let Ok((client, _)) = accepted else { break };
                                let proxy = target.proxy;
                                let host = target.route.remote_host.clone();
                                let port = target.route.remote_port;
                                let counters = traffic.counters.clone();
                                connections.spawn(async move {
                                    let _ = forward(client, proxy, &host, port, counters).await;
                                });
                            }
                            Some(_) = connections.join_next(), if !connections.is_empty() => {}
                        }
                    }
                })
            })
            .collect();
        Ok(Self { tasks, traffic })
    }

    pub fn traffic(&self) -> Vec<RelayTraffic> {
        self.traffic.clone()
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
    client: TcpStream,
    proxy: SocketAddr,
    remote_host: &str,
    remote_port: u16,
    counters: Arc<Counters>,
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
    counters.active.fetch_add(1, Ordering::Relaxed);
    let _established = Established(counters.clone());
    let mut client = CountedStream {
        stream: client,
        counters: counters.clone(),
        tx: false,
    };
    let mut upstream = CountedStream {
        stream: upstream,
        counters,
        tx: true,
    };
    tokio::io::copy_bidirectional(&mut client, &mut upstream)
        .await
        .map_err(|_| error("VPN relay connection ended unexpectedly."))?;
    Ok(())
}
