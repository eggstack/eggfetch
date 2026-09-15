#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::large_futures
)]

//! Tests for the Tower adapter over the native frame-preserving body path.

use std::convert::Infallible;
use std::fmt;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{AuthScheme, Client, NativeHttpService, NativeRequestOptions, Timeout};
use http_body::{Body, Frame};
use http_body_util::{BodyExt, Empty, StreamBody};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower_service::Service;

#[derive(Debug)]
struct ServiceBodyError;

impl fmt::Display for ServiceBodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic service request body failure")
    }
}

impl std::error::Error for ServiceBodyError {}

async fn start_response_server(
    response: &'static [u8],
) -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = socket.read(&mut buffer).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        socket.write_all(response).await.unwrap();
        socket.flush().await.unwrap();
        request
    });
    (format!("http://{address}/"), task)
}

fn assert_service_ready<B>(service: &mut NativeHttpService)
where
    B: Body<Data = Bytes> + Send + 'static,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let waker = futures_util::task::noop_waker_ref();
    let mut context = std::task::Context::from_waker(waker);
    assert!(
        <NativeHttpService as Service<http::Request<B>>>::poll_ready(service, &mut context)
            .is_ready()
    );
}

#[tokio::test]
async fn service_preserves_request_and_response_frames() {
    let (url, server) = start_response_server(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: x-response-trailer\r\n\r\n3\r\none\r\n3\r\ntwo\r\n0\r\nx-response-trailer: yes\r\n\r\n",
    )
    .await;
    let request_trailers = http::HeaderMap::from_iter([(
        http::header::HeaderName::from_static("x-request-trailer"),
        http::HeaderValue::from_static("present"),
    )]);
    let body = StreamBody::new(futures_util::stream::iter([
        Ok::<_, Infallible>(Frame::data(Bytes::from_static(b"hello "))),
        Ok(Frame::data(Bytes::from_static(b"world"))),
        Ok(Frame::trailers(request_trailers)),
    ]));
    let request = http::Request::post(&url)
        .header("trailer", "x-request-trailer")
        .body(body)
        .unwrap();

    let mut service = Client::new().native_service();
    let response = service.call(request).await.unwrap();
    let mut data = Vec::new();
    let mut trailers = None;
    let mut body = Box::pin(response.into_body());
    while let Some(frame) = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        let frame = frame.unwrap();
        match frame.into_data() {
            Ok(bytes) => data.extend_from_slice(&bytes),
            Err(frame) => trailers = frame.into_trailers().ok(),
        }
    }
    assert_eq!(data, b"onetwo");
    assert_eq!(trailers.unwrap().get("x-response-trailer").unwrap(), "yes");

    let request = server.await.unwrap();
    assert!(request.windows(5).any(|window| window == b"hello"));
    assert!(request.windows(5).any(|window| window == b"world"));
    assert!(request
        .windows(17)
        .any(|window| window == b"x-request-trailer"));
}

#[tokio::test]
async fn constructors_are_behaviorally_equivalent() {
    let client = Client::new();
    let mut from_client = client.native_service();
    let mut from_constructor = NativeHttpService::new(client);

    for service in [&mut from_client, &mut from_constructor] {
        let (url, server) =
            start_response_server(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n").await;
        let request = http::Request::get(url).body(Empty::<Bytes>::new()).unwrap();
        assert_service_ready::<Empty<Bytes>>(service);
        assert_eq!(service.call(request).await.unwrap().status(), 204);
        server.await.unwrap();
    }
}

#[tokio::test]
async fn readiness_is_always_ready_and_does_not_consume_pool_admission() {
    let (url, server) =
        start_response_server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
    let client = Client::builder().max_in_flight_requests(1).build();
    let mut service = client.native_service();
    let mut clone = service.clone();
    let waker = futures_util::task::noop_waker_ref();
    let mut context = std::task::Context::from_waker(waker);
    for _ in 0..8 {
        assert!(
            <NativeHttpService as Service<http::Request<Empty<Bytes>>>>::poll_ready(
                &mut service,
                &mut context
            )
            .is_ready()
        );
        assert!(
            <NativeHttpService as Service<http::Request<Empty<Bytes>>>>::poll_ready(
                &mut clone,
                &mut context
            )
            .is_ready()
        );
    }
    let request = http::Request::get(url).body(Empty::<Bytes>::new()).unwrap();
    let response = tokio::time::timeout(Duration::from_secs(2), service.call(request))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "ok"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn with_options_propagates_native_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    });
    let body = StreamBody::new(futures_util::stream::pending::<
        Result<Frame<Bytes>, Infallible>,
    >());
    let request = http::Request::post(format!("http://{address}/"))
        .body(body)
        .unwrap();
    let options = NativeRequestOptions::default().timeout(Timeout {
        write: Some(Duration::from_millis(50)),
        ..Timeout::default()
    });
    let mut service = NativeHttpService::new(Client::new()).with_options(options);
    let error = service.call(request).await.unwrap_err();
    assert_eq!(error.kind(), "timeout_write");
    server.abort();
}

#[tokio::test]
async fn cloned_services_share_pool_state() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            let _ = socket.read(&mut request).await;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                .await
                .unwrap();
        }
    });
    let client = Client::builder().max_in_flight_requests(1).build();
    let mut first_service = client.native_service();
    let mut second_service = first_service.clone();
    let first_request = http::Request::get(format!("http://{address}/first"))
        .body(Empty::<Bytes>::new())
        .unwrap();
    let first_response = first_service.call(first_request).await.unwrap();
    let second_request = http::Request::get(format!("http://{address}/second"))
        .body(Empty::<Bytes>::new())
        .unwrap();
    let mut second = tokio::spawn(async move { second_service.call(second_request).await });
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut second)
            .await
            .is_err()
    );
    drop(first_response);
    let response = tokio::time::timeout(Duration::from_secs(2), second)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "ok"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn service_preserves_native_body_error_mapping() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
    });
    let body = StreamBody::new(futures_util::stream::iter([
        Ok::<_, ServiceBodyError>(Frame::data(Bytes::from_static(b"one"))),
        Err(ServiceBodyError),
    ]));
    let request = http::Request::post(format!("http://{address}/"))
        .body(body)
        .unwrap();
    let mut service = Client::new().native_service();
    let error = tokio::time::timeout(Duration::from_secs(2), service.call(request))
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error.kind(), "hyper_client" | "body"));
    server.abort();
}

#[tokio::test]
async fn service_does_not_apply_high_level_redirect_or_auth_policy() {
    let (url, server) = start_response_server(
        b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\n\r\n",
    )
    .await;
    let client = Client::builder()
        .follow_redirects(true)
        .auth(AuthScheme::bearer("service-secret").unwrap())
        .build();
    let mut service = client.native_service();
    let request = http::Request::get(url).body(Empty::<Bytes>::new()).unwrap();
    let response = service.call(request).await.unwrap();
    assert_eq!(response.status(), 302);
    assert!(response.headers().get("authorization").is_none());

    let request = server.await.unwrap();
    assert!(!request
        .windows(b"service-secret".len())
        .any(|window| window == b"service-secret"));
}
