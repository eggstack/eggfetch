//! Loopback coverage for the native Rust JSON and response-limit APIs.

#![cfg(feature = "json")]
#![allow(
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::type_complexity
)]

#[cfg(feature = "compression-gzip")]
use std::io::Write;

use eggfetch_core::{Client, Error};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct FailingJson;

impl serde::Serialize for FailingJson {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("secret payload"))
    }
}

async fn read_request(stream: &mut TcpStream) -> (String, Vec<u8>) {
    let mut request = Vec::new();
    loop {
        let mut byte = [0; 1];
        stream.read_exact(&mut byte).await.unwrap();
        request.push(byte[0]);
        if request.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header_end = request.len();
    let raw_headers = String::from_utf8_lossy(&request[..header_end]).into_owned();
    let headers = raw_headers.to_ascii_lowercase();
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.unwrap();
    (raw_headers, body)
}

async fn response_server(
    body: Vec<u8>,
    extra_headers: &str,
) -> (String, tokio::task::JoinHandle<(String, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let extra_headers = extra_headers.to_owned();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (request_headers, request_body) = read_request(&mut stream).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
            body.len(),
            extra_headers
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.write_all(&body).await.unwrap();
        (request_headers, request_body)
    });
    (format!("http://{address}/"), handle)
}

async fn redirect_server() -> (String, tokio::task::JoinHandle<(Vec<u8>, Vec<u8>)>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let url = format!("http://{address}/start");
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (_, first_body) = read_request(&mut stream).await;
        let redirect = format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{address}/final\r\n\
             Content-Length: 0\r\nConnection: keep-alive\r\n\r\n"
        );
        stream.write_all(redirect.as_bytes()).await.unwrap();
        let (_, second_body) = read_request(&mut stream).await;
        let body = br#"{"ok":true}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        stream.write_all(response.as_bytes()).await.unwrap();
        stream.write_all(body).await.unwrap();
        (first_body, second_body)
    });
    (url, handle)
}

#[tokio::test]
async fn request_json_sends_serialized_replayable_body_and_default_media_type() {
    let (url, server) = response_server(br#"{"ok":true}"#.to_vec(), "").await;
    let client = Client::new();
    let mut response = client
        .post(&url)
        .unwrap()
        .json(&serde_json::json!({"name": "Ada"}))
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"ok": true})
    );
    let (headers, body) = server.await.unwrap();
    assert!(headers
        .to_ascii_lowercase()
        .contains("content-type: application/json"));
    assert_eq!(body, br#"{"name":"Ada"}"#);
}

#[tokio::test]
async fn request_json_preserves_explicit_media_type() {
    let (url, server) = response_server(br#"{"ok":true}"#.to_vec(), "").await;
    let client = Client::new();
    let mut response = client
        .post(&url)
        .unwrap()
        .header("content-type", "application/vnd.api+json")
        .json(&serde_json::json!({"name": "Ada"}))
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"ok": true})
    );
    let (headers, _) = server.await.unwrap();
    assert!(headers
        .to_ascii_lowercase()
        .contains("content-type: application/vnd.api+json"));
}

#[tokio::test]
async fn request_json_serialization_fails_before_network_io() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let error = Client::new()
        .post(&url)
        .unwrap()
        .json(&FailingJson)
        .err()
        .expect("test value cannot be serialized as JSON");
    assert_eq!(error.kind(), "json_serialize");
    assert!(!error.to_string().contains("secret payload"));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(20), listener.accept())
            .await
            .is_err(),
        "serialization must complete before a connection can be opened"
    );
}

#[tokio::test]
async fn response_json_consumes_body_once_and_does_not_require_media_type() {
    let (url, server) =
        response_server(br#"{"ok":true}"#.to_vec(), "Content-Type: text/plain\r\n").await;
    let mut response = Client::new().get(&url).unwrap().send().await.unwrap();
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"ok": true})
    );
    assert!(matches!(response.bytes().await, Err(Error::Body(_))));
    let _ = server.await.unwrap();
}

#[tokio::test]
async fn request_json_body_replays_across_a_permitted_redirect() {
    let (url, server) = redirect_server().await;
    let mut response = Client::builder()
        .follow_redirects(true)
        .max_decoded_body_size(1)
        .build()
        .post(&url)
        .unwrap()
        .json(&serde_json::json!({"name": "Ada"}))
        .unwrap()
        .max_decoded_body_size(64)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"ok": true})
    );
    let (first_body, second_body) = server.await.unwrap();
    assert_eq!(first_body, second_body);
}

#[tokio::test]
async fn request_limit_override_takes_precedence_and_applies_to_json() {
    let (url, server) = response_server(br#"{"ok":true}"#.to_vec(), "").await;
    let client = Client::builder().max_decoded_body_size(1).build();
    let mut response = client
        .get(&url)
        .unwrap()
        .max_decoded_body_size(64)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"ok": true})
    );
    let _ = server.await.unwrap();

    let (url, server) = response_server(br#"{"ok":true}"#.to_vec(), "").await;
    let mut response = Client::new()
        .get(&url)
        .unwrap()
        .max_decoded_body_size(1)
        .send()
        .await
        .unwrap();
    assert!(matches!(
        response.json::<serde_json::Value>().await,
        Err(Error::DecodedBodyTooLarge)
    ));
    let _ = server.await.unwrap();
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn compressed_json_uses_the_normal_decode_path() {
    let mut compressed = Vec::new();
    {
        let mut encoder =
            flate2::write::GzEncoder::new(&mut compressed, flate2::Compression::default());
        encoder.write_all(br#"{"ok":true}"#).unwrap();
        encoder.finish().unwrap();
    }
    let (url, server) = response_server(compressed, "Content-Encoding: gzip\r\n").await;
    let mut response = Client::builder()
        .max_decompression_ratio(0.01)
        .build()
        .get(&url)
        .unwrap()
        .max_decompression_ratio(1000.0)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.json::<serde_json::Value>().await.unwrap(),
        serde_json::json!({"ok": true})
    );
    let _ = server.await.unwrap();
}

#[tokio::test]
async fn response_json_reports_parse_errors_without_echoing_payload() {
    let (url, server) = response_server(b"{not-json".to_vec(), "").await;
    let mut response = Client::new().get(&url).unwrap().send().await.unwrap();
    let error = response.json::<serde_json::Value>().await.unwrap_err();
    assert_eq!(error.kind(), "json_deserialize");
    assert!(!error.to_string().contains("not-json"));
    let _ = server.await.unwrap();

    let (url, server) = response_server(vec![0xff], "").await;
    let mut response = Client::new().get(&url).unwrap().send().await.unwrap();
    let error = response.json::<serde_json::Value>().await.unwrap_err();
    assert_eq!(error.kind(), "json_deserialize");
    let _ = server.await.unwrap();
}
