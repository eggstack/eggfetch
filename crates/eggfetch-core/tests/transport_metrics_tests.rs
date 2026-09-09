#![allow(
    missing_docs,
    dead_code,
    clippy::missing_panics_doc,
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Transport observability exact-count tests.
//!
//! `TransportMetrics` counts connector events and protocol connections
//! where observable; `PoolMetrics` counts logical permits. Hyper
//! socket-reuse counts remain absent (not estimated).

use std::net::SocketAddr;
use std::time::Duration;

use eggfetch_core::Client;

async fn start_http_server(body: &'static [u8]) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = vec![0u8; 4096];
            let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf)).await;
            let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(body).await;
            let _ = socket.flush().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    (format!("http://{addr}/"), handle)
}

#[tokio::test]
async fn direct_connector_success_counts_exactly() {
    let (url, server) = start_http_server(b"ok").await;
    let client = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .build();
    let before = client.transport_metrics().snapshot();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let _ = resp.bytes().await.unwrap();
    let after = client.transport_metrics().snapshot();
    assert_eq!(
        after.direct_connector_attempts - before.direct_connector_attempts,
        1
    );
    assert_eq!(
        after.direct_connector_successes - before.direct_connector_successes,
        1
    );
    assert_eq!(
        after.direct_connector_failures - before.direct_connector_failures,
        0
    );
    assert_eq!(after.direct_dns_attempts - before.direct_dns_attempts, 1);
    assert_eq!(after.direct_dns_failures - before.direct_dns_failures, 0);
    // Pool metrics remain the logical-permit authority (no waits here).
    assert_eq!(
        client
            .pool_metrics()
            .acquisition_waits
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    server.abort();
}

#[tokio::test]
async fn direct_connector_failure_counts_exactly() {
    // Closed loopback port: fast TCP refusal, deterministic.
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let url = format!("http://127.0.0.1:{port}/");
    let client = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();
    let before = client.transport_metrics().snapshot();
    let _ = client.get(&url).unwrap().send().await;
    let after = client.transport_metrics().snapshot();
    assert_eq!(
        after.direct_connector_attempts - before.direct_connector_attempts,
        1
    );
    assert_eq!(
        after.direct_connector_failures - before.direct_connector_failures,
        1
    );
    assert_eq!(
        after.direct_connector_successes - before.direct_connector_successes,
        0
    );
}

#[cfg(unix)]
#[tokio::test]
async fn uds_success_counts_exactly() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let path = "/tmp/eggfetch_metrics_uds.sock";
    let _ = std::fs::remove_file(path);
    let shutdown = Arc::new(AtomicBool::new(false));
    let sd = shutdown.clone();
    let sp = path.to_owned();
    let handle = std::thread::spawn(move || {
        let listener = UnixListener::bind(&sp).unwrap();
        listener.set_nonblocking(true).unwrap();
        while !sd.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf).unwrap_or(0);
                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
                    let _ = stream.flush();
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let client = Client::builder().uds_path(path.to_owned()).build();
    let before = client.transport_metrics().snapshot();
    let mut resp = client
        .get("http://localhost/")
        .unwrap()
        .send()
        .await
        .unwrap();
    let _ = resp.bytes().await.unwrap();
    let after = client.transport_metrics().snapshot();
    assert_eq!(
        after.uds_connector_attempts - before.uds_connector_attempts,
        1
    );
    assert_eq!(
        after.uds_connector_failures - before.uds_connector_failures,
        0
    );
    shutdown.store(true, Ordering::SeqCst);
    let _ = handle.join();
    let _ = std::fs::remove_file(path);
}

#[cfg(unix)]
#[tokio::test]
async fn uds_failure_counts_exactly() {
    let client = Client::builder()
        .uds_path("/tmp/eggfetch_metrics_uds_missing.sock".to_owned())
        .build();
    let before = client.transport_metrics().snapshot();
    let _ = client.get("http://localhost/").unwrap().send().await;
    let after = client.transport_metrics().snapshot();
    assert_eq!(
        after.uds_connector_attempts - before.uds_connector_attempts,
        1
    );
    assert_eq!(
        after.uds_connector_failures - before.uds_connector_failures,
        1
    );
}

#[cfg(feature = "proxy")]
#[tokio::test]
async fn proxy_failure_counts_exactly() {
    use eggfetch_core::Proxy;
    // Closed proxy port: deterministic failure without origin I/O.
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);
    let client = Client::builder()
        .proxy(Proxy::all(&format!("http://127.0.0.1:{port}")).unwrap())
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();
    let before = client.transport_metrics().snapshot();
    let _ = client.get("http://example.com/").unwrap().send().await;
    let after = client.transport_metrics().snapshot();
    assert_eq!(
        after.proxy_connector_attempts - before.proxy_connector_attempts,
        1
    );
    assert_eq!(
        after.proxy_connector_failures - before.proxy_connector_failures,
        1
    );
}

#[tokio::test]
async fn upgraded_count_increments_on_101() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut acc = Vec::new();
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            loop {
                let mut tmp = [0u8; 1];
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        acc.push(tmp[0]);
                        if acc.len() >= 4 && &acc[acc.len() - 4..] == b"\r\n\r\n" {
                            break;
                        }
                    }
                }
            }
            let _ = stream.write_all(
                b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: echo\r\nConnection: Upgrade\r\n\r\n",
            );
            let _ = stream.flush();
            std::thread::sleep(Duration::from_millis(300));
        }
    });
    let url = format!("http://127.0.0.1:{port}/");
    let client = Client::builder()
        .local_address("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .build();
    let before = client.transport_metrics().snapshot();
    let resp = client
        .get(&url)
        .unwrap()
        .header("upgrade", "echo")
        .header("connection", "Upgrade")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 101);
    assert!(resp.network_stream().is_some());
    let after = client.transport_metrics().snapshot();
    assert_eq!(
        after.upgraded_connections_created - before.upgraded_connections_created,
        1,
        "101 capture counts as protocol connection"
    );
}
