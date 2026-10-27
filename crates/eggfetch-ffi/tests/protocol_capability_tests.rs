#![allow(
    missing_docs,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines
)]

//! Fail-closed behavior for FFI HTTP-version policies the linked build
//! cannot honor.
//!
//! `eggfetch_client_builder_http_version` exposes a user-facing protocol
//! control. A strict policy that needs a capability the linked library does
//! not have must be rejected before any connection is attempted, with an
//! error that names the missing capability. `Auto` is not strict: it
//! negotiates down to the protocols the build has and stays accepted.
//!
//! The published policy values are `0` (`Http1Only`), `1` (`Http2Only`),
//! `2` (`Auto` without HTTP/3), `3` (`Auto` with HTTP/3), and `4`
//! (`Http3Only`). Values `1` and `4` are strict, so each one names the
//! capability the build lacks and fails closed; `Auto` is not strict and
//! never fails closed.
//!
//! Neither `http2` nor `http3` is in the FFI crate's default feature set,
//! so a default build rejects both strict values. That is the fail-closed
//! contract: a consumer that needs HTTP/2 or HTTP/3 builds `eggfetch-ffi`
//! with the matching feature. The strict HTTP/2 rejection is observable at
//! the ABI boundary below; the strict HTTP/3 rejection is asserted against
//! the same ABI surface.

use std::ffi::CString;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::raw::c_char;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Wording the capability failure must not reuse.
const GENERIC_MARKER: &str = "HTTP client not available";

/// A local HTTP/1.1 server that counts accepted connections.
struct CountingServer {
    port: u16,
    accepted: Arc<AtomicUsize>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl CountingServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
        listener
            .set_nonblocking(true)
            .expect("failed to set nonblocking");
        let port = listener.local_addr().expect("local addr").port();
        let shutdown = Arc::new(AtomicBool::new(false));
        let accepted = Arc::new(AtomicUsize::new(0));

        let sd = shutdown.clone();
        let ac = accepted.clone();
        let handle = thread::spawn(move || {
            while !sd.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        ac.fetch_add(1, Ordering::SeqCst);
                        thread::spawn(move || {
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                            let mut buffer = [0_u8; 1024];
                            let _ = stream.read(&mut buffer);
                            let _ =
                                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
                        });
                    }
                    Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            port,
            accepted,
            shutdown,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    fn accepted_connections(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

impl Drop for CountingServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Copy a C string out of the FFI and release it.
fn take_string(pointer: *mut c_char) -> String {
    assert!(!pointer.is_null(), "the FFI returned a null string");
    unsafe {
        let value = std::ffi::CStr::from_ptr(pointer)
            .to_string_lossy()
            .into_owned();
        eggfetch_ffi::eggfetch_string_free(pointer);
        value
    }
}

/// Send a request built from `url` and return the error message, if any.
fn send_and_read_error(client: *mut eggfetch_ffi::ClientHandle, url: &str) -> String {
    let url = CString::new(url).expect("url");
    unsafe {
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null(), "the request handle must be created");
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        assert!(response.is_null(), "the request must be rejected");
        assert!(!err.is_null(), "the rejection must report an error");
        let kind = take_string(eggfetch_ffi::eggfetch_error_kind(err));
        assert_eq!(kind, "unsupported", "the rejection must be unsupported");
        let message = take_string(eggfetch_ffi::eggfetch_error_message(err));
        eggfetch_ffi::eggfetch_error_free(err);
        message
    }
}

#[test]
#[cfg(not(feature = "http2"))]
fn strict_http2_policy_is_rejected_and_names_http2() {
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 1),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let message = send_and_read_error(client, &server.url());
        assert!(message.contains("HTTP/2"), "{message}");
        assert!(!message.contains(GENERIC_MARKER), "{message}");
        eggfetch_ffi::eggfetch_client_free(client);
    }
    assert_eq!(
        server.accepted_connections(),
        0,
        "a rejected protocol request must not open a connection"
    );
}

#[test]
#[cfg(not(feature = "http3"))]
fn auto_with_http3_is_accepted_without_the_http3_capability() {
    // `Auto` is not a strict request: value 3 is `Auto { allow_http3: true }`
    // and must keep negotiating down to the protocols this build has.
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 3),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let url = CString::new(server.url()).expect("url");
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        assert!(!response.is_null(), "auto negotiation must not fail");
        assert_eq!(eggfetch_ffi::eggfetch_response_status(response), 200);
        eggfetch_ffi::eggfetch_response_free(response);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn auto_policy_is_accepted_and_the_request_succeeds() {
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 2),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 3),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let url = CString::new(server.url()).expect("url");
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        assert!(!response.is_null(), "auto negotiation must not fail");
        assert_eq!(eggfetch_ffi::eggfetch_response_status(response), 200);
        eggfetch_ffi::eggfetch_response_free(response);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn http1_policy_is_accepted_and_the_request_succeeds() {
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 0),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let url = CString::new(server.url()).expect("url");
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        assert!(!response.is_null(), "HTTP/1.1 must not fail");
        eggfetch_ffi::eggfetch_response_free(response);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
#[cfg(not(feature = "http3"))]
fn strict_http3_policy_is_rejected_and_names_http3() {
    // Value `4` is `Http3Only`: a strict request that needs the `http3`
    // capability this build does not have. The ABI accepts the value; the
    // request fails closed and names the missing capability.
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 4),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let message = send_and_read_error(client, &server.url());
        assert!(message.contains("HTTP/3"), "{message}");
        assert!(!message.contains(GENERIC_MARKER), "{message}");
        eggfetch_ffi::eggfetch_client_free(client);
    }
    assert_eq!(
        server.accepted_connections(),
        0,
        "a rejected protocol request must not open a connection"
    );
}

#[test]
#[cfg(feature = "http3")]
fn strict_http3_policy_is_not_rejected_with_the_capability() {
    // With the `http3` feature compiled in, the same ABI value must not be
    // rejected by the capability guard.
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 4),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let url = CString::new(server.url()).expect("url");
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        if response.is_null() {
            let message = take_string(eggfetch_ffi::eggfetch_error_message(err));
            eggfetch_ffi::eggfetch_error_free(err);
            assert!(
                !message.contains("is not supported by this build"),
                "the capability guard must not reject HTTP/3: {message}"
            );
            assert!(!message.contains(GENERIC_MARKER), "{message}");
        } else {
            eggfetch_ffi::eggfetch_response_free(response);
        }
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn unknown_policy_value_is_rejected() {
    // The published value set is `0..=4`. Anything above it is unknown and
    // must still fail at the ABI boundary rather than silently defaulting.
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 5),
            -1
        );
        eggfetch_ffi::eggfetch_client_builder_free(builder);
    }
}

#[test]
fn default_policy_is_unchanged() {
    let server = CountingServer::start();
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        assert!(!client.is_null());
        let url = CString::new(server.url()).expect("url");
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        assert!(!response.is_null(), "the default policy must keep working");
        eggfetch_ffi::eggfetch_response_free(response);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
#[cfg(feature = "http2")]
fn strict_http2_policy_is_honored_with_the_capability() {
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(eggfetch_ffi::eggfetch_client_builder_timeout(builder, 5), 0);
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 1),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let url = CString::new(server.url()).expect("url");
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        if response.is_null() {
            // The fixture speaks HTTP/1.1, so an HTTP/2-only request still
            // fails — but never with the capability rejection.
            let message = take_string(eggfetch_ffi::eggfetch_error_message(err));
            eggfetch_ffi::eggfetch_error_free(err);
            assert!(
                !message.contains("is not supported by this build"),
                "the capability guard must not reject HTTP/2: {message}"
            );
            assert!(!message.contains(GENERIC_MARKER), "{message}");
        } else {
            eggfetch_ffi::eggfetch_response_free(response);
        }
        eggfetch_ffi::eggfetch_client_free(client);
    }
    assert_eq!(
        server.accepted_connections(),
        1,
        "the request must be attempted when the capability is compiled in"
    );
}

#[test]
#[cfg(feature = "http3")]
fn auto_with_http3_is_accepted_with_the_capability() {
    let server = CountingServer::start();
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_connect_timeout(builder, 2),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 3),
            0
        );
        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_builder_free(builder);

        let url = CString::new(server.url()).expect("url");
        let request = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!request.is_null());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let response = eggfetch_ffi::eggfetch_client_send(client, request, &raw mut err);
        if response.is_null() {
            let message = take_string(eggfetch_ffi::eggfetch_error_message(err));
            eggfetch_ffi::eggfetch_error_free(err);
            assert!(
                !message.contains("is not supported by this build"),
                "the capability guard must not reject an accepted policy: {message}"
            );
        } else {
            eggfetch_ffi::eggfetch_response_free(response);
        }
        eggfetch_ffi::eggfetch_client_free(client);
    }
}
