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
