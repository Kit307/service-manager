use service_manager::{
    model::VpnRoute,
    vpn_relay::{RelayTarget, VpnRelaySet},
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

async fn free_port() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    listener.local_addr().unwrap().port()
}

async fn proxy(status: &'static str) -> SocketAddr {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut client, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut request = Vec::new();
                let mut byte = [0u8; 1];
                while request.len() < 8192 && client.read_exact(&mut byte).await.is_ok() {
                    request.push(byte[0]);
                    if request.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                if status != "200 Connection Established" {
                    let _ = client
                        .write_all(format!("HTTP/1.1 {status}\r\n\r\n").as_bytes())
                        .await;
                    return;
                }
                let request = String::from_utf8(request).unwrap();
                let destination = request
                    .lines()
                    .next()
                    .unwrap()
                    .strip_prefix("CONNECT ")
                    .unwrap()
                    .strip_suffix(" HTTP/1.1")
                    .unwrap();
                let mut upstream = TcpStream::connect(destination).await.unwrap();
                client
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .await
                    .unwrap();
                let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
            });
        }
    });
    address
}

fn route(local_port: u16, remote_port: u16) -> VpnRoute {
    VpnRoute {
        profile_id: "ac9c00d2-4810-43b6-a8e9-b9f07fecf6ec".into(),
        remote_host: "127.0.0.1".into(),
        remote_port,
        local_port,
    }
}

#[tokio::test]
async fn loopback_relay_carries_tcp_through_connect_proxy() {
    let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let target_port = target.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = target.accept().await.unwrap();
        let mut bytes = [0u8; 4];
        stream.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes, b"ping");
        stream.write_all(b"pong").await.unwrap();
    });
    let local_port = free_port().await;
    let relay = VpnRelaySet::start(vec![RelayTarget {
        route: route(local_port, target_port),
        proxy: proxy("200 Connection Established").await,
    }])
    .await
    .unwrap();

    let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, local_port))
        .await
        .unwrap();
    client.write_all(b"ping").await.unwrap();
    let mut bytes = [0u8; 4];
    client.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"pong");
    drop(relay);
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, local_port))
        .await
        .is_err());
}

#[tokio::test]
async fn relay_rejects_occupied_ports_before_starting_any_listener() {
    let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let occupied_port = occupied.local_addr().unwrap().port();
    let free = free_port().await;
    let result = VpnRelaySet::start(vec![
        RelayTarget {
            route: route(free, 443),
            proxy: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9),
        },
        RelayTarget {
            route: route(occupied_port, 443),
            proxy: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9),
        },
    ])
    .await;
    assert!(result.is_err());
    assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, free)).await.is_ok());
}

#[tokio::test]
async fn relay_does_not_fall_back_when_proxy_rejects_destination() {
    let local_port = free_port().await;
    let relay = VpnRelaySet::start(vec![RelayTarget {
        route: route(local_port, 443),
        proxy: proxy("502 Bad Gateway").await,
    }])
    .await
    .unwrap();
    let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, local_port))
        .await
        .unwrap();
    client.write_all(b"must-not-pass").await.unwrap();
    let mut byte = [0u8; 1];
    assert!(matches!(client.read(&mut byte).await, Ok(0) | Err(_)));
    let snapshot = relay.traffic()[0].snapshot();
    assert_eq!(
        (
            snapshot.tx_bytes,
            snapshot.rx_bytes,
            snapshot.active_connections
        ),
        (0, 0, 0)
    );
}

#[tokio::test]
async fn traffic_tracks_live_payloads_and_independent_routes() {
    let target_a = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let target_b = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let local_a = free_port().await;
    let local_b = free_port().await;
    let proxy = proxy("200 Connection Established").await;
    let route_a = route(local_a, target_a.local_addr().unwrap().port());
    let relay = VpnRelaySet::start(vec![
        RelayTarget {
            route: route_a.clone(),
            proxy,
        },
        RelayTarget {
            route: route(local_b, target_b.local_addr().unwrap().port()),
            proxy,
        },
    ])
    .await
    .unwrap();
    let traffic = relay.traffic();
    let handle = traffic[0].clone();
    let mut client_a = TcpStream::connect((Ipv4Addr::LOCALHOST, local_a))
        .await
        .unwrap();
    let (mut remote_a, _) = target_a.accept().await.unwrap();
    let mut client_b = TcpStream::connect((Ipv4Addr::LOCALHOST, local_b))
        .await
        .unwrap();
    let (mut remote_b, _) = target_b.accept().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while traffic.iter().any(|t| t.snapshot().active_connections != 1) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let idle = handle.snapshot();
    assert_eq!(idle.route.local_port, route_a.local_port);
    assert_eq!(
        (idle.tx_bytes, idle.rx_bytes, idle.active_connections),
        (0, 0, 1)
    );
    client_a.write_all(b"hello").await.unwrap();
    let mut bytes = [0; 5];
    remote_a.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"hello");
    remote_b.write_all(b"ok").await.unwrap();
    let mut reply = [0; 2];
    client_b.read_exact(&mut reply).await.unwrap();
    assert_eq!(&reply, b"ok");
    let a = handle.snapshot();
    let b = traffic[1].snapshot();
    assert_eq!((a.tx_bytes, a.rx_bytes, a.active_connections), (5, 0, 1));
    assert_eq!((b.tx_bytes, b.rx_bytes, b.active_connections), (0, 2, 1));
    drop(client_a);
    drop(remote_a);
    drop(client_b);
    drop(remote_b);
    tokio::time::timeout(Duration::from_secs(2), async {
        while traffic.iter().any(|t| t.snapshot().active_connections != 0) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn half_close_keeps_reverse_payload_and_traffic() {
    let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let local_port = free_port().await;
    let relay = VpnRelaySet::start(vec![RelayTarget {
        route: route(local_port, target.local_addr().unwrap().port()),
        proxy: proxy("200 Connection Established").await,
    }])
    .await
    .unwrap();
    let traffic = relay.traffic().remove(0);
    let mut client = TcpStream::connect((Ipv4Addr::LOCALHOST, local_port))
        .await
        .unwrap();
    let (mut remote, _) = target.accept().await.unwrap();
    client.write_all(b"request").await.unwrap();
    client.shutdown().await.unwrap();
    let mut request = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), remote.read_to_end(&mut request))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request, b"request");
    remote.write_all(b"response").await.unwrap();
    remote.shutdown().await.unwrap();
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(2), client.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response, b"response");
    let snapshot = traffic.snapshot();
    assert_eq!((snapshot.tx_bytes, snapshot.rx_bytes), (7, 8));
}

#[tokio::test]
async fn active_connections_decrement_independently_and_on_relay_drop() {
    let target = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let local_port = free_port().await;
    let relay = VpnRelaySet::start(vec![RelayTarget {
        route: route(local_port, target.local_addr().unwrap().port()),
        proxy: proxy("200 Connection Established").await,
    }])
    .await
    .unwrap();
    let traffic = relay.traffic().remove(0);
    let client_a = TcpStream::connect((Ipv4Addr::LOCALHOST, local_port))
        .await
        .unwrap();
    let (remote_a, _) = target.accept().await.unwrap();
    let _client_b = TcpStream::connect((Ipv4Addr::LOCALHOST, local_port))
        .await
        .unwrap();
    let (_remote_b, _) = target.accept().await.unwrap();
    async fn wait_for(traffic: &service_manager::vpn_relay::RelayTraffic, count: u64) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while traffic.snapshot().active_connections != count {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
    wait_for(&traffic, 2).await;
    drop(client_a);
    drop(remote_a);
    wait_for(&traffic, 1).await;
    drop(relay);
    wait_for(&traffic, 0).await;
}
