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
    tasks: Vec<(Option<tokio::sync::oneshot::Sender<()>>, JoinHandle<()>)>,
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

struct CountedStream<S> {
    stream: S,
    counters: Arc<Counters>,
    tx: bool,
}

impl<S: AsyncRead + Unpin> AsyncRead for CountedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for CountedStream<S> {
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
                let (stop, mut stopped) = tokio::sync::oneshot::channel();
                let task = tokio::spawn(async move {
                    let mut connections = JoinSet::new();
                    loop {
                        tokio::select! {
                            _ = &mut stopped => break,
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
                    drop(listener);
                    connections.shutdown().await;
                });
                (Some(stop), task)
            })
            .collect();
        Ok(Self { tasks, traffic })
    }

    pub async fn shutdown(mut self) {
        for (stop, _) in &mut self.tasks {
            if let Some(stop) = stop.take() {
                let _ = stop.send(());
            }
        }
        while let Some((_, task)) = self.tasks.last_mut() {
            let _ = task.await;
            self.tasks.pop();
        }
    }

    pub fn traffic(&self) -> Vec<RelayTraffic> {
        self.traffic.clone()
    }
}

impl Drop for VpnRelaySet {
    fn drop(&mut self) {
        for (_, task) in &self.tasks {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, task::Wake};

    struct ScriptedWriter(VecDeque<Poll<io::Result<usize>>>);

    impl AsyncWrite for ScriptedWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.0.pop_front().unwrap()
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    struct NoopWake;
    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }

    #[test]
    fn counts_only_successful_written_bytes() {
        let counters = Arc::new(Counters::default());
        let mut stream = CountedStream {
            stream: ScriptedWriter(VecDeque::from([
                Poll::Ready(Ok(2)),
                Poll::Pending,
                Poll::Ready(Ok(3)),
                Poll::Ready(Ok(0)),
                Poll::Ready(Err(io::Error::other("write failed"))),
            ])),
            counters: counters.clone(),
            tx: true,
        };
        let waker = std::task::Waker::from(Arc::new(NoopWake));
        let mut cx = Context::from_waker(&waker);
        let input = b"abcdefgh";
        assert!(matches!(
            Pin::new(&mut stream).poll_write(&mut cx, input),
            Poll::Ready(Ok(2))
        ));
        assert_eq!(counters.tx.load(Ordering::Relaxed), 2);
        assert!(matches!(
            Pin::new(&mut stream).poll_write(&mut cx, input),
            Poll::Pending
        ));
        assert_eq!(counters.tx.load(Ordering::Relaxed), 2);
        assert!(matches!(
            Pin::new(&mut stream).poll_write(&mut cx, input),
            Poll::Ready(Ok(3))
        ));
        assert_eq!(counters.tx.load(Ordering::Relaxed), 5);
        assert!(matches!(
            Pin::new(&mut stream).poll_write(&mut cx, input),
            Poll::Ready(Ok(0))
        ));
        assert!(matches!(
            Pin::new(&mut stream).poll_write(&mut cx, input),
            Poll::Ready(Err(_))
        ));
        assert_eq!(counters.tx.load(Ordering::Relaxed), 5);
        assert_eq!(counters.rx.load(Ordering::Relaxed), 0);
    }
}
