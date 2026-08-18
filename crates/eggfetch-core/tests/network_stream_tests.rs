#![allow(
    missing_docs,
    dead_code,
    unused_mut,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls,
    clippy::inefficient_to_string,
    clippy::manual_let_else,
    clippy::single_char_pattern,
    clippy::match_same_arms,
    clippy::needless_borrow,
    clippy::trim_split_whitespace,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::items_after_statements,
    clippy::expect_fun_call,
    clippy::len_zero,
    clippy::unnecessary_debug_formatting,
    clippy::format_push_string,
    clippy::new_without_default,
    clippy::map_unwrap_or
)]
//! Tests for network stream, upgraded stream, and connection metadata.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use eggfetch_core::network_stream::{
    ConnectionMetadata, ExtraInfo, NetworkStream, TlsInfo, TransportKind, UpgradedStream,
};

/// Test server that sends a 101 Switching Protocols response and
/// echoes bytes bidirectionally. Reads have a 1-second timeout to
/// detect client close promptly.
fn start_upgrade_server() -> (u16, Arc<std::sync::atomic::AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sd = shutdown.clone();

    thread::spawn(move || {
        while !sd.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok((mut stream, _)) = listener.accept() {
                stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
                stream.set_write_timeout(Some(Duration::from_secs(1))).ok();

                // Read request line and headers.
                let mut buf = Vec::new();
                let mut tmp = [0u8; 1];
                let mut headers_done = false;
                while let Ok(n) = Read::read(&mut stream, &mut tmp) {
                    if n == 0 {
                        break;
                    }
                    buf.push(tmp[0]);
                    if buf.len() >= 4 && &buf[buf.len() - 4..] == b"\r\n\r\n" {
                        headers_done = true;
                        break;
                    }
                }

                if !headers_done {
                    continue;
                }

                // Send 101 response with leading application bytes.
                let response = "HTTP/1.1 101 Switching Protocols\r\n\
                               Upgrade: echo\r\n\
                               Connection: Upgrade\r\n\
                               \r\n";
                let leading = b"LEADING";
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.write_all(leading);
                let _ = stream.flush();

                // Echo loop with short timeout to detect close.
                let mut echo_buf = [0u8; 1024];
                loop {
                    match Read::read(&mut stream, &mut echo_buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if stream.write_all(&echo_buf[..n]).is_err() {
                                break;
                            }
                            if stream.flush().is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    });

    (port, shutdown)
}

#[tokio::test]
async fn upgrade_101_response_has_network_stream() {
    let (port, _shutdown) = start_upgrade_server();
    let url = format!("http://127.0.0.1:{port}/");

    let client = eggfetch_core::Client::new();
    let mut response = client
        .get(&url)
        .unwrap()
        .header("upgrade", "echo")
        .header("connection", "Upgrade")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 101);

    // The response should have a network stream.
    assert!(response.network_stream().is_some());
    let ns = response.network_stream().unwrap();
    assert!(ns.is_upgraded());

    // Get the upgraded stream and verify metadata.
    let upgraded = ns.as_upgraded().unwrap();
    let metadata = upgraded.metadata();
    assert_eq!(metadata.transport_kind, TransportKind::Tcp);
}

#[tokio::test]
async fn upgraded_stream_read_write_roundtrip() {
    let (port, _shutdown) = start_upgrade_server();
    let url = format!("http://127.0.0.1:{port}/");

    let client = eggfetch_core::Client::new();
    let mut response = client
        .get(&url)
        .unwrap()
        .header("upgrade", "echo")
        .header("connection", "Upgrade")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 101);

    // Extract the upgraded stream.
    let mut ns = response.into_network_stream().expect("network stream");
    let mut upgraded = ns.into_upgraded().expect("upgraded stream");

    // Hyper preserves leading data in its internal rewind buffer.
    // Read and discard it before the roundtrip test.
    let _leading = upgraded.read(1024).await.unwrap();

    // Write data and read it back.
    let test_data = b"Hello, upgraded world!";
    upgraded.write_all(test_data).await.unwrap();

    let data = upgraded.read(1024).await.unwrap();
    assert_eq!(&data[..], test_data);

    // Close the stream.
    upgraded.close().await.unwrap();
}

#[tokio::test]
async fn upgraded_stream_partial_reads() {
    let (port, _shutdown) = start_upgrade_server();
    let url = format!("http://127.0.0.1:{port}/");

    let client = eggfetch_core::Client::new();
    let mut response = client
        .get(&url)
        .unwrap()
        .header("upgrade", "echo")
        .header("connection", "Upgrade")
        .send()
        .await
        .unwrap();

    let mut ns = response.into_network_stream().expect("network stream");
    let mut upgraded = ns.into_upgraded().expect("upgraded stream");

    // Drain leading data first.
    let _leading = upgraded.read(1024).await.unwrap();

    // Write a large payload and read it back in small chunks.
    let large_data = vec![0xABu8; 10_000];
    upgraded.write_all(&large_data).await.unwrap();

    let mut received = Vec::new();
    while received.len() < large_data.len() {
        let chunk = upgraded.read(256).await.unwrap();
        if chunk.is_empty() {
            break;
        }
        received.extend_from_slice(&chunk);
    }
    assert_eq!(received, large_data);
}

#[tokio::test]
async fn upgraded_stream_close_is_idempotent() {
    let (port, _shutdown) = start_upgrade_server();
    let url = format!("http://127.0.0.1:{port}/");

    let client = eggfetch_core::Client::new();
    let mut response = client
        .get(&url)
        .unwrap()
        .header("upgrade", "echo")
        .header("connection", "Upgrade")
        .send()
        .await
        .unwrap();

    let mut ns = response.into_network_stream().expect("network stream");
    let mut upgraded = ns.into_upgraded().expect("upgraded stream");

    // Close multiple times should not panic.
    upgraded.close().await.unwrap();
    upgraded.close().await.unwrap();
}

#[test]
fn network_stream_metadata_accessors() {
    let meta = Arc::new(ConnectionMetadata {
        local_addr: Some("127.0.0.1:12345".parse().unwrap()),
        peer_addr: Some("93.184.216.34:443".parse().unwrap()),
        transport_kind: TransportKind::Tls,
        tls_info: Some(TlsInfo {
            alpn_protocol: Some("h2".into()),
            tls_version: Some("TLSv1.3".into()),
            cipher_suite: Some("TLS_AES_256_GCM_SHA384".into()),
            server_name: Some("example.com".into()),
        }),
    });
    let ns = NetworkStream::Metadata(meta);
    assert!(!ns.is_upgraded());
    assert!(ns.as_upgraded().is_none());
    let info = ns.metadata();
    assert_eq!(info.local_addr.unwrap().port(), 12345);
    assert_eq!(info.peer_addr.unwrap().port(), 443);
}

#[test]
fn extra_info_from_metadata() {
    let meta = ConnectionMetadata {
        local_addr: Some("127.0.0.1:1000".parse().unwrap()),
        peer_addr: Some("10.0.0.1:443".parse().unwrap()),
        transport_kind: TransportKind::Tcp,
        tls_info: None,
    };
    let info = ExtraInfo::from_metadata(&meta);
    assert_eq!(info.client_addr.unwrap().port(), 1000);
    assert_eq!(info.server_addr.unwrap().port(), 443);
    assert!(info.tls_info.is_none());
}

#[tokio::test]
async fn upgraded_stream_leading_data() {
    let leading = bytes::Bytes::from("hello leading");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let _handle = tokio::task::spawn_blocking(move || {
        let _ = std::net::TcpStream::connect(addr);
    });
    let (std_stream, _) = listener.accept().unwrap();
    std_stream.set_nonblocking(true).ok();
    let tokio_stream = tokio::net::TcpStream::from_std(std_stream).unwrap();
    let mut us = UpgradedStream::from_tcp(tokio_stream, leading.clone());
    assert_eq!(us.leading_data(), &leading);
    let taken = us.take_leading_data();
    assert_eq!(taken, leading);
    assert!(us.leading_data().is_empty());
}

#[test]
fn connection_metadata_default() {
    let meta = ConnectionMetadata::default();
    assert!(meta.local_addr.is_none());
    assert!(meta.peer_addr.is_none());
    assert_eq!(meta.transport_kind, TransportKind::Tcp);
    assert!(meta.tls_info.is_none());
}
