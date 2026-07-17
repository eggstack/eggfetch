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
//! Integration tests for the eggfetch FFI crate.

use std::ffi::CString;
use std::ptr;

#[test]
fn client_new_and_free() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_free(client);
        eggfetch_ffi::eggfetch_client_free(ptr::null_mut());
    }
}

#[test]
fn request_header_and_query() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://example.com/test").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!req.is_null());

        let name = CString::new("X-Custom").unwrap();
        let value = CString::new("test-value").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_request_header(req, name.as_ptr(), value.as_ptr()),
            0
        );

        let qkey = CString::new("key").unwrap();
        let qval = CString::new("val").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_request_query(req, qkey.as_ptr(), qval.as_ptr()),
            0
        );

        eggfetch_ffi::eggfetch_request_free(req);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn request_body_str() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://example.com/post").unwrap();
        let req = eggfetch_ffi::eggfetch_client_post(client, url.as_ptr());
        assert!(!req.is_null());

        let body = CString::new("hello world").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_request_body_str(req, body.as_ptr()),
            0
        );

        eggfetch_ffi::eggfetch_request_free(req);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn request_body_bytes() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://example.com/post").unwrap();
        let req = eggfetch_ffi::eggfetch_client_post(client, url.as_ptr());
        assert!(!req.is_null());

        let data = b"binary data";
        assert_eq!(
            eggfetch_ffi::eggfetch_request_body(req, data.as_ptr(), data.len()),
            0
        );

        eggfetch_ffi::eggfetch_request_free(req);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn null_safety() {
    unsafe {
        let url = CString::new("http://example.com").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(ptr::null(), url.as_ptr());
        assert!(req.is_null());

        let client = eggfetch_ffi::eggfetch_client_new();
        let req = eggfetch_ffi::eggfetch_client_get(client, ptr::null());
        assert!(req.is_null());

        let req = eggfetch_ffi::eggfetch_client_request(client, ptr::null(), url.as_ptr());
        assert!(req.is_null());

        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send(ptr::null(), req, &mut err);
        assert!(resp.is_null());
        assert!(!err.is_null());
        eggfetch_ffi::eggfetch_error_free(err);

        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn error_handle() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://invalid.example.test:99999/nope").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send(client, req, &mut err);

        if resp.is_null() {
            assert!(!err.is_null());
            assert_eq!(eggfetch_ffi::eggfetch_error_is_error(err), 1);

            let kind = eggfetch_ffi::eggfetch_error_kind(err);
            assert!(!kind.is_null());
            eggfetch_ffi::eggfetch_string_free(kind);

            let msg = eggfetch_ffi::eggfetch_error_message(err);
            assert!(!msg.is_null());
            eggfetch_ffi::eggfetch_string_free(msg);

            eggfetch_ffi::eggfetch_error_free(err);
        } else {
            eggfetch_ffi::eggfetch_response_free(resp);
        }

        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn error_is_error_null() {
    unsafe {
        assert_eq!(eggfetch_ffi::eggfetch_error_is_error(ptr::null()), 0);
    }
}

#[test]
fn response_null_safety() {
    unsafe {
        assert_eq!(eggfetch_ffi::eggfetch_response_status(ptr::null()), 0);
        assert!(eggfetch_ffi::eggfetch_response_url(ptr::null()).is_null());
        assert_eq!(eggfetch_ffi::eggfetch_response_header_count(ptr::null()), 0);
        assert_eq!(eggfetch_ffi::eggfetch_response_is_success(ptr::null()), 0);
    }
}

#[test]
fn convenience_methods() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://example.com").unwrap();
        let methods: Vec<unsafe extern "C" fn(_, _) -> _> = vec![
            eggfetch_ffi::eggfetch_client_get,
            eggfetch_ffi::eggfetch_client_post,
            eggfetch_ffi::eggfetch_client_put,
            eggfetch_ffi::eggfetch_client_patch,
            eggfetch_ffi::eggfetch_client_delete,
            eggfetch_ffi::eggfetch_client_head,
            eggfetch_ffi::eggfetch_client_options,
        ];

        for method_fn in methods {
            let req = method_fn(client, url.as_ptr());
            assert!(!req.is_null());
            eggfetch_ffi::eggfetch_request_free(req);
        }

        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn string_free_null() {
    unsafe {
        eggfetch_ffi::eggfetch_string_free(ptr::null_mut());
    }
}

#[test]
fn full_request_response() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let _server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let mut buf = vec![0u8; 4096];
        let _n = stream.read(&mut buf).unwrap();

        let response_body = b"Hello, FFI!";
        let header = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain\r\n\
             X-Test: hello\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n",
            response_body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(response_body);
        let _ = stream.flush();
    });

    let url = format!("http://{addr}/test");
    let url_c = CString::new(url).unwrap();

    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let req = eggfetch_ffi::eggfetch_client_get(client, url_c.as_ptr());
        assert!(!req.is_null());

        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send(client, req, &mut err);

        if resp.is_null() {
            let err_msg = if err.is_null() {
                "no error".to_owned()
            } else {
                let msg = eggfetch_ffi::eggfetch_error_message(err);
                let s = if msg.is_null() {
                    "?".to_owned()
                } else {
                    std::ffi::CStr::from_ptr(msg).to_string_lossy().into_owned()
                };
                eggfetch_ffi::eggfetch_string_free(msg);
                eggfetch_ffi::eggfetch_error_free(err);
                s
            };
            panic!("request returned null: err={err_msg}");
        }

        assert_eq!(eggfetch_ffi::eggfetch_response_status(resp), 200);
        assert_eq!(eggfetch_ffi::eggfetch_response_is_success(resp), 1);

        let header_count = eggfetch_ffi::eggfetch_response_header_count(resp);
        assert!(header_count >= 2);

        let mut found_test = false;
        for i in 0..header_count {
            let mut name: *mut std::os::raw::c_char = ptr::null_mut();
            let mut value: *mut std::os::raw::c_char = ptr::null_mut();
            let rc = eggfetch_ffi::eggfetch_response_header(resp, i, &mut name, &mut value);
            assert_eq!(rc, 0);
            let name_str = CString::from_raw(name).into_string().unwrap();
            let value_str = CString::from_raw(value).into_string().unwrap();
            if name_str.eq_ignore_ascii_case("x-test") {
                assert_eq!(value_str, "hello");
                found_test = true;
            }
        }
        assert!(found_test, "should find X-Test header");

        eggfetch_ffi::eggfetch_response_free(resp);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

/// Helper: start a local TCP server that reads one request and responds
/// with the given body. Returns the bound address.
fn start_echo_server(response_body: &[u8]) -> std::net::SocketAddr {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let body = response_body.to_vec();

    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let mut buf = vec![0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();

        let header = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
        let _ = stream.flush();
    });

    addr
}

/// Helper: start a local TCP server that responds with chunked transfer encoding.
fn start_chunked_server() -> std::net::SocketAddr {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let mut buf = vec![0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();

        let response = "\
            HTTP/1.1 200 OK\r\n\
            Content-Type: text/plain\r\n\
            Transfer-Encoding: chunked\r\n\
            Connection: close\r\n\
            \r\n\
            5\r\n\
            Hello\r\n\
            6\r\n\
            , worl\r\n\
            2\r\n\
            d!\r\n\
            0\r\n\
            \r\n";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    addr
}

#[test]
fn client_builder_lifecycle() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert!(!builder.is_null());

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());

        eggfetch_ffi::eggfetch_client_free(client);
        eggfetch_ffi::eggfetch_client_builder_free(ptr::null_mut());
    }
}

#[test]
fn client_builder_timeout() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_timeout(builder, 30),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_connect_timeout(builder, 5),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_read_timeout(builder, 10),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_write_timeout(builder, 10),
            0
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());

        let url = CString::new("http://example.com").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!req.is_null());
        eggfetch_ffi::eggfetch_request_free(req);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn client_builder_redirects() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_follow_redirects(builder, 1),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_max_redirects(builder, 10),
            0
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn client_builder_user_agent() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        let agent = CString::new("eggfetch-ffi/0.1").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_user_agent(builder, agent.as_ptr()),
            0
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());

        let url = CString::new("http://example.com").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!req.is_null());
        eggfetch_ffi::eggfetch_request_free(req);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn client_builder_http_version() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 0),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, 2),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_http_version(builder, -1),
            -1
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn client_builder_decompression() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_automatic_decompression(builder, 1),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_max_decoded_body_size(builder, 1024 * 1024),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_max_decompression_ratio(builder, 20.0),
            0
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn client_builder_pool() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_max_idle_connections(builder, 100),
            0
        );
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_max_idle_connections_per_host(builder, 10),
            0
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn client_builder_insecure_tls() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_danger_accept_invalid_certs(builder, 1),
            0
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn client_builder_auth() {
    unsafe {
        let builder = eggfetch_ffi::eggfetch_client_builder_new();
        let user = CString::new("admin").unwrap();
        let pass = CString::new("secret").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_basic_auth(builder, user.as_ptr(), pass.as_ptr()),
            0
        );

        let builder2 = eggfetch_ffi::eggfetch_client_builder_new();
        let token = CString::new("my-token").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_client_builder_bearer_auth(builder2, token.as_ptr()),
            0
        );

        let client = eggfetch_ffi::eggfetch_client_builder_build(builder);
        assert!(!client.is_null());
        eggfetch_ffi::eggfetch_client_free(client);

        let client2 = eggfetch_ffi::eggfetch_client_builder_build(builder2);
        assert!(!client2.is_null());
        eggfetch_ffi::eggfetch_client_free(client2);
    }
}

#[test]
fn request_per_request_timeout() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://example.com").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert!(!req.is_null());

        assert_eq!(eggfetch_ffi::eggfetch_request_timeout(req, 5), 0);

        eggfetch_ffi::eggfetch_request_free(req);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn request_per_request_auth() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://example.com").unwrap();

        let req1 = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        let user = CString::new("user").unwrap();
        let pass = CString::new("pass").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_request_auth_basic(req1, user.as_ptr(), pass.as_ptr()),
            0
        );
        eggfetch_ffi::eggfetch_request_free(req1);

        let req2 = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        let token = CString::new("tok").unwrap();
        assert_eq!(
            eggfetch_ffi::eggfetch_request_auth_bearer(req2, token.as_ptr()),
            0
        );
        eggfetch_ffi::eggfetch_request_free(req2);

        let req3 = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        assert_eq!(eggfetch_ffi::eggfetch_request_without_auth(req3), 0);
        eggfetch_ffi::eggfetch_request_free(req3);

        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn request_decompress_and_redirect() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://example.com").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());

        assert_eq!(eggfetch_ffi::eggfetch_request_decompress(req, 0), 0);
        assert_eq!(eggfetch_ffi::eggfetch_request_redirect_policy(req, 1, 5), 0);

        eggfetch_ffi::eggfetch_request_free(req);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn streaming_response_basic() {
    let body = b"Hello, streaming!";
    let addr = start_echo_server(body);

    let url = format!("http://{addr}/test");
    let url_c = CString::new(url).unwrap();

    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let req = eggfetch_ffi::eggfetch_client_get(client, url_c.as_ptr());
        assert!(!req.is_null());

        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send_streaming(client, req, &mut err);

        if resp.is_null() {
            let err_msg = if err.is_null() {
                "no error".to_owned()
            } else {
                let msg = eggfetch_ffi::eggfetch_error_message(err);
                let s = if msg.is_null() {
                    "?".to_owned()
                } else {
                    std::ffi::CStr::from_ptr(msg).to_string_lossy().into_owned()
                };
                eggfetch_ffi::eggfetch_string_free(msg);
                eggfetch_ffi::eggfetch_error_free(err);
                s
            };
            panic!("streaming request returned null: err={err_msg}");
        }

        assert_eq!(eggfetch_ffi::eggfetch_response_stream_status(resp), 200);

        let mut collected = Vec::new();
        loop {
            let chunk = eggfetch_ffi::eggfetch_response_stream_next(resp);
            if chunk.is_null() {
                break;
            }
            let chunk_ref = &*chunk;
            if !chunk_ref.data.is_null() && chunk_ref.len > 0 {
                let slice = std::slice::from_raw_parts(chunk_ref.data, chunk_ref.len);
                collected.extend_from_slice(slice);
            }
            eggfetch_ffi::eggfetch_stream_chunk_free(chunk);
        }

        assert_eq!(collected, body);

        eggfetch_ffi::eggfetch_response_stream_free(resp);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn streaming_response_chunked() {
    let addr = start_chunked_server();

    let url = format!("http://{addr}/test");
    let url_c = CString::new(url).unwrap();

    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let req = eggfetch_ffi::eggfetch_client_get(client, url_c.as_ptr());
        assert!(!req.is_null());

        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send_streaming(client, req, &mut err);

        if resp.is_null() {
            let err_msg = if err.is_null() {
                "no error".to_owned()
            } else {
                let msg = eggfetch_ffi::eggfetch_error_message(err);
                let s = if msg.is_null() {
                    "?".to_owned()
                } else {
                    std::ffi::CStr::from_ptr(msg).to_string_lossy().into_owned()
                };
                eggfetch_ffi::eggfetch_string_free(msg);
                eggfetch_ffi::eggfetch_error_free(err);
                s
            };
            panic!("streaming chunked request returned null: err={err_msg}");
        }

        assert_eq!(eggfetch_ffi::eggfetch_response_stream_status(resp), 200);

        let mut collected = Vec::new();
        loop {
            let chunk = eggfetch_ffi::eggfetch_response_stream_next(resp);
            if chunk.is_null() {
                break;
            }
            let chunk_ref = &*chunk;
            if !chunk_ref.data.is_null() && chunk_ref.len > 0 {
                let slice = std::slice::from_raw_parts(chunk_ref.data, chunk_ref.len);
                collected.extend_from_slice(slice);
            }
            eggfetch_ffi::eggfetch_stream_chunk_free(chunk);
        }

        assert_eq!(collected, b"Hello, world!");

        eggfetch_ffi::eggfetch_response_stream_free(resp);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn streaming_response_headers() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let _server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let mut buf = vec![0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();

        let response = "\
            HTTP/1.1 200 OK\r\n\
            Content-Type: text/plain\r\n\
            X-Custom: streaming-test\r\n\
            Transfer-Encoding: chunked\r\n\
            Connection: close\r\n\
            \r\n\
            5\r\n\
            Hello\r\n\
            0\r\n\
            \r\n";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    let url = format!("http://{addr}/test");
    let url_c = CString::new(url).unwrap();

    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let req = eggfetch_ffi::eggfetch_client_get(client, url_c.as_ptr());

        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send_streaming(client, req, &mut err);

        assert!(!resp.is_null(), "streaming headers test: request failed");

        assert_eq!(eggfetch_ffi::eggfetch_response_stream_status(resp), 200);

        let header_count = eggfetch_ffi::eggfetch_response_stream_header_count(resp);
        assert!(header_count >= 2);

        let mut found_custom = false;
        for i in 0..header_count {
            let mut name: *mut std::os::raw::c_char = ptr::null_mut();
            let mut value: *mut std::os::raw::c_char = ptr::null_mut();
            let rc = eggfetch_ffi::eggfetch_response_stream_header(resp, i, &mut name, &mut value);
            assert_eq!(rc, 0);
            let name_str = CString::from_raw(name).into_string().unwrap();
            let value_str = CString::from_raw(value).into_string().unwrap();
            if name_str.eq_ignore_ascii_case("x-custom") {
                assert_eq!(value_str, "streaming-test");
                found_custom = true;
            }
        }
        assert!(found_custom, "should find X-Custom header");

        // Drain the body
        while !eggfetch_ffi::eggfetch_response_stream_next(resp).is_null() {}

        eggfetch_ffi::eggfetch_response_stream_free(resp);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn streaming_cancel() {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    // Server that streams slowly
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    let _server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
        let mut buf = vec![0u8; 4096];
        let _ = stream.read(&mut buf).unwrap();

        let response = "\
            HTTP/1.1 200 OK\r\n\
            Content-Type: text/plain\r\n\
            Transfer-Encoding: chunked\r\n\
            Connection: close\r\n\
            \r\n\
            5\r\n\
            Hello\r\n\
            0\r\n\
            \r\n";
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    let url = format!("http://{addr}/test");
    let url_c = CString::new(url).unwrap();

    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let req = eggfetch_ffi::eggfetch_client_get(client, url_c.as_ptr());

        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send_streaming(client, req, &mut err);

        assert!(!resp.is_null(), "streaming cancel test: request failed");

        // Cancel immediately
        eggfetch_ffi::eggfetch_response_stream_cancel(resp);

        // Subsequent reads should return null
        let chunk = eggfetch_ffi::eggfetch_response_stream_next(resp);
        assert!(chunk.is_null());

        eggfetch_ffi::eggfetch_response_stream_free(resp);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn streaming_null_safety() {
    unsafe {
        assert_eq!(
            eggfetch_ffi::eggfetch_response_stream_status(ptr::null()),
            0
        );
        assert!(eggfetch_ffi::eggfetch_response_stream_url(ptr::null()).is_null());
        assert_eq!(
            eggfetch_ffi::eggfetch_response_stream_header_count(ptr::null()),
            0
        );
        assert!(eggfetch_ffi::eggfetch_response_stream_next(ptr::null_mut()).is_null());
        eggfetch_ffi::eggfetch_response_stream_free(ptr::null_mut());
        eggfetch_ffi::eggfetch_stream_chunk_free(ptr::null_mut());
        eggfetch_ffi::eggfetch_response_stream_cancel(ptr::null_mut());
    }
}

#[test]
fn error_handle_message_and_kind() {
    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let url = CString::new("http://invalid.example.test:99999/nope").unwrap();
        let req = eggfetch_ffi::eggfetch_client_get(client, url.as_ptr());
        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send(client, req, &mut err);

        if resp.is_null() {
            assert!(!err.is_null());

            let kind = eggfetch_ffi::eggfetch_error_kind(err);
            assert!(!kind.is_null());
            let kind_str = CString::from_raw(kind);
            assert!(!kind_str.to_string_lossy().is_empty());

            let msg = eggfetch_ffi::eggfetch_error_message(err);
            assert!(!msg.is_null());
            let msg_str = CString::from_raw(msg);
            assert!(!msg_str.to_string_lossy().is_empty());

            eggfetch_ffi::eggfetch_error_free(err);
        } else {
            eggfetch_ffi::eggfetch_response_free(resp);
        }

        eggfetch_ffi::eggfetch_client_free(client);
    }
}

#[test]
fn response_body_text() {
    let addr = start_echo_server(b"hello text");

    let url = format!("http://{addr}/test");
    let url_c = CString::new(url).unwrap();

    unsafe {
        let client = eggfetch_ffi::eggfetch_client_new();
        let req = eggfetch_ffi::eggfetch_client_get(client, url_c.as_ptr());

        let mut err: *mut eggfetch_ffi::ErrorHandle = ptr::null_mut();
        let resp = eggfetch_ffi::eggfetch_client_send(client, req, &mut err);

        assert!(!resp.is_null());

        let text = eggfetch_ffi::eggfetch_response_text(resp);
        assert!(!text.is_null());
        let text_str = CString::from_raw(text);
        assert_eq!(text_str.to_string_lossy().as_ref(), "hello text");

        eggfetch_ffi::eggfetch_response_free(resp);
        eggfetch_ffi::eggfetch_client_free(client);
    }
}
