#![allow(
    missing_docs,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::large_futures
)]

//! Tests for the native frame-preserving body boundary.

use std::convert::Infallible;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, NativeRequestOptions, Timeout};
use http_body::{Body, Frame};
use http_body_util::{BodyExt, StreamBody};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
struct TestBodyError;

impl fmt::Display for TestBodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic request body failure")
    }
}

impl std::error::Error for TestBodyError {}

struct OneFrameThenPendingBody {
    polls: Arc<AtomicUsize>,
    sent: bool,
}

impl Body for OneFrameThenPendingBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if self.sent {
            std::task::Poll::Pending
        } else {
            self.sent = true;
            std::task::Poll::Ready(Some(Ok(Frame::data(Bytes::from_static(b"one")))))
        }
    }

    fn is_end_stream(&self) -> bool {
        false
    }

    fn size_hint(&self) -> http_body::SizeHint {
        http_body::SizeHint::default()
    }
}

async fn start_frame_server() -> (String, tokio::task::JoinHandle<Vec<u8>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buffer))
                .await
                .unwrap()
                .unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if let Some(position) = request.windows(3).rposition(|window| window == b"0\r\n") {
                if request[position + 3..]
                    .windows(4)
                    .any(|window| window == b"\r\n\r\n")
                {
                    break;
                }
            }
        }
        let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: x-response-trailer\r\n\r\n3\r\none\r\n3\r\ntwo\r\n0\r\nx-response-trailer: yes\r\n\r\n";
        socket.write_all(response).await.unwrap();
        socket.flush().await.unwrap();
        request
    });
    (format!("http://{address}/"), task)
}

#[tokio::test]
async fn native_body_preserves_data_and_trailer_frames() {
    let (url, server) = start_frame_server().await;
    let mut request_trailers = http::HeaderMap::new();
    request_trailers.insert("x-request-trailer", "present".parse().unwrap());
    let body = StreamBody::new(futures_util::stream::iter([
        Ok::<_, Infallible>(Frame::data(Bytes::from_static(b"hello "))),
        Ok(Frame::data(Bytes::from_static(b"world"))),
        Ok(Frame::trailers(request_trailers)),
    ]));
    let request = http::Request::post(&url)
        .header("trailer", "x-request-trailer")
        .body(body)
        .unwrap();
    let client = Client::builder().build();
    let response = client
        .execute_http_body(request, NativeRequestOptions::default())
        .await
        .unwrap();

    let mut data = Vec::new();
    let mut trailers = None;
    let mut body = Box::pin(response.into_body());
    while let Some(frame) = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        let frame = frame.unwrap();
        match frame.into_data() {
            Ok(bytes) => data.extend_from_slice(&bytes),
            Err(frame) => {
                if let Ok(headers) = frame.into_trailers() {
                    trailers = Some(headers);
                }
            }
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
async fn dropping_native_body_releases_logical_pool_lease() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
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
    let url = format!("http://{address}/");
    let client = Client::builder().max_in_flight_requests(1).build();
    let request = http::Request::get(&url)
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();
    let response = client.execute_http_body_default(request).await.unwrap();
    drop(response);

    let request = http::Request::get(&url)
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        client.execute_http_body_default(request),
    )
    .await
    .unwrap()
    .unwrap();
    let body = response.into_body();
    assert_eq!(body.collect().await.unwrap().to_bytes(), "ok");
    task.await.unwrap();
}

#[tokio::test]
async fn native_body_request_errors_surface_without_replay() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
    });
    let body = StreamBody::new(futures_util::stream::iter([
        Ok::<_, TestBodyError>(Frame::data(Bytes::from_static(b"one"))),
        Err(TestBodyError),
    ]));
    let request = http::Request::post(format!("http://{address}/"))
        .body(body)
        .unwrap();
    let error = tokio::time::timeout(
        Duration::from_secs(2),
        Client::new().execute_http_body_default(request),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert!(matches!(error.kind(), "hyper_client" | "body"));
    task.abort();
}

#[tokio::test]
async fn native_request_body_is_polled_incrementally() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !request.windows(3).any(|window| window == b"one") {
            let read = socket.read(&mut buffer).await.unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let polls = Arc::new(AtomicUsize::new(0));
    let body = OneFrameThenPendingBody {
        polls: polls.clone(),
        sent: false,
    };
    let request = http::Request::post(format!("http://{address}/"))
        .body(body)
        .unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        Client::new().execute_http_body_default(request),
    )
    .await
    .unwrap()
    .unwrap();
    // The body remains pending after its first frame. Hyper may poll that
    // pending state more than once while the server responds, but it must not
    // wait for the body to produce EOF before returning response headers.
    assert!(polls.load(Ordering::SeqCst) >= 2);
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "ok"
    );
    task.await.unwrap();
}

#[tokio::test]
async fn native_body_rejects_relative_uri_before_network_io() {
    let client = Client::new();
    let request = http::Request::get("/relative")
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();
    let error = client.execute_http_body_default(request).await.unwrap_err();
    assert_eq!(error.kind(), "invalid_url");
}

#[tokio::test(flavor = "current_thread")]
async fn native_body_enforces_request_write_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
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
    let error = Client::new()
        .execute_http_body(
            request,
            NativeRequestOptions::default().timeout(Timeout {
                write: Some(Duration::from_millis(50)),
                ..Timeout::default()
            }),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), "timeout_write");
    task.abort();
}

#[tokio::test(flavor = "current_thread")]
async fn native_body_enforces_response_read_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
            .await
            .unwrap();
        socket.flush().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
    });
    let request = http::Request::get(format!("http://{address}/"))
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();
    let response = Client::new()
        .execute_http_body(
            request,
            NativeRequestOptions::default().timeout(Timeout {
                read: Some(Duration::from_millis(50)),
                ..Timeout::default()
            }),
        )
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    let first = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.into_data().unwrap(), Bytes::from_static(b"ab"));
    let error = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(error.kind(), "timeout_read");
    task.abort();
}

#[tokio::test(flavor = "current_thread")]
async fn native_response_read_timeout_starts_on_first_body_poll() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = socket.read(&mut request).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n")
            .await
            .unwrap();
        socket.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(180)).await;
        socket.write_all(b"ok").await.unwrap();
    });
    let request = http::Request::get(format!("http://{address}/"))
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();
    let response = Client::new()
        .execute_http_body(
            request,
            NativeRequestOptions::default().timeout(Timeout {
                read: Some(Duration::from_millis(100)),
                ..Timeout::default()
            }),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(120)).await;
    let body = response.into_body();
    let bytes = tokio::time::timeout(Duration::from_millis(250), body.collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    assert_eq!(bytes, "ok");
    task.await.unwrap();
}

#[tokio::test]
async fn native_response_error_releases_logical_pool_lease() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 1024];
        let _ = first.read(&mut request).await;
        first
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
            .await
            .unwrap();
        drop(first);

        let (mut second, _) = listener.accept().await.unwrap();
        let _ = second.read(&mut request).await;
        second
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let client = Client::builder().max_in_flight_requests(1).build();
    let request = http::Request::get(format!("http://{address}/bad"))
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();
    let response = client.execute_http_body_default(request).await.unwrap();
    let mut body = Box::pin(response.into_body());
    let first = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.into_data().unwrap(), Bytes::from_static(b"ab"));
    let error = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(error.kind(), "hyper" | "hyper_client" | "body"));
    drop(body);

    let request = http::Request::get(format!("http://{address}/good"))
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        client.execute_http_body_default(request),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "ok"
    );
    task.await.unwrap();
}
