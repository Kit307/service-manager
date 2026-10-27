use service_manager::{
    model::VpnRoute,
    vpn_relay::{RelayTarget, VpnRelaySet},
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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
    let _relay = VpnRelaySet::start(vec![RelayTarget {
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
}
