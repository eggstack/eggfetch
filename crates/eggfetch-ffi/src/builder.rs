//! Client builder FFI functions.
//!
//! Provides fine-grained control over client configuration before building.

use std::ptr;

use eggfetch_core::Client;

use crate::handle::ClientBuilderHandle;

/// Create a new client builder with default settings.
///
/// Returns null on allocation failure. Caller must free with [`eggfetch_client_builder_free`].
///
/// # Safety
///
/// Caller must free the returned handle with [`eggfetch_client_builder_free`].
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_new() -> *mut ClientBuilderHandle {
    Box::into_raw(Box::new(ClientBuilderHandle(Client::builder())))
}

/// Free a client builder handle.
///
/// # Safety
///
/// `handle` must have been returned by [`eggfetch_client_builder_new`] and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_free(handle: *mut ClientBuilderHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

/// Build a client from the configured builder.
///
/// Consumes the builder. Returns null on allocation failure.
/// Caller must free with [`crate::handle::eggfetch_client_free`].
///
/// # Safety
///
/// - `handle` must be a valid, non-freed builder handle.
/// - The builder is consumed (freed) by this call.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_build(
    raw: *mut ClientBuilderHandle,
) -> *mut crate::handle::ClientHandle {
    let Some(handle) = raw.as_mut() else {
        return ptr::null_mut();
    };
    let builder = std::ptr::read(&handle.0);
    // Leak the box to prevent its destructor from running on the moved-out value.
    // `raw` came from Box::into_raw, so forgetting it is safe and only leaks
    // the small handle allocation (acceptable for FFI consume-once semantics).
    std::mem::forget(Box::from_raw(raw));
    let client = builder.build();
    Box::into_raw(Box::new(crate::handle::ClientHandle(client)))
}

/// Set the overall request timeout in seconds.
///
/// This is the total time limit for a complete request-response cycle,
/// including connection, sending, and receiving.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_timeout(
    handle: *mut ClientBuilderHandle,
    secs: u64,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(
        &mut handle.0,
        builder.timeout(eggfetch_core::Timeout::from_secs(secs)),
    );
    0
}

/// Set the connect timeout in seconds.
///
/// Limits how long to wait for the TCP/TLS connection to establish.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_connect_timeout(
    handle: *mut ClientBuilderHandle,
    secs: u64,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let timeout = eggfetch_core::Timeout::builder()
        .connect(std::time::Duration::from_secs(secs))
        .build();
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.timeout(timeout));
    0
}

/// Set the read timeout in seconds.
///
/// Limits how long to wait for a response body chunk.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_read_timeout(
    handle: *mut ClientBuilderHandle,
    secs: u64,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let timeout = eggfetch_core::Timeout::builder()
        .read(std::time::Duration::from_secs(secs))
        .build();
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.timeout(timeout));
    0
}

/// Set the write timeout in seconds.
///
/// Limits how long to wait to send the request body.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_write_timeout(
    handle: *mut ClientBuilderHandle,
    secs: u64,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let timeout = eggfetch_core::Timeout::builder()
        .write(std::time::Duration::from_secs(secs))
        .build();
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.timeout(timeout));
    0
}

/// Enable or disable automatic redirect following.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_follow_redirects(
    handle: *mut ClientBuilderHandle,
    follow: i32,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.follow_redirects(follow != 0));
    0
}

/// Set the maximum number of redirects to follow.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_max_redirects(
    handle: *mut ClientBuilderHandle,
    max: usize,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.max_redirects(max));
    0
}

/// Set the User-Agent header.
///
/// Returns 0 on success, -1 if handle or agent are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed builder handle.
/// - `agent` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_user_agent(
    handle: *mut ClientBuilderHandle,
    agent: *const std::os::raw::c_char,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let Some(agent_str) = crate::handle::cstr_to_opt(agent) else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.user_agent(agent_str));
    0
}

/// Add a default header to all requests.
///
/// Returns 0 on success, -1 if handle or arguments are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed builder handle.
/// - `name` and `value` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_default_header(
    handle: *mut ClientBuilderHandle,
    name: *const std::os::raw::c_char,
    value: *const std::os::raw::c_char,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let Some(name_str) = crate::handle::cstr_to_opt(name) else {
        return -1;
    };
    let Some(value_str) = crate::handle::cstr_to_opt(value) else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    match builder.default_header(name_str, value_str) {
        Ok(b) => {
            std::ptr::write(&mut handle.0, b);
            0
        }
        Err(_) => -1,
    }
}

/// Set the HTTP version policy.
///
/// `policy` values:
/// - 0: `Http1Only`
/// - 1: `Http2Only`
/// - 2: `Auto` (`allow_http3` = false)
/// - 3: `Auto` (`allow_http3` = true)
///
/// Returns 0 on success, -1 if handle is invalid or policy value is unknown.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_http_version(
    handle: *mut ClientBuilderHandle,
    policy: i32,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let version_policy = match policy {
        0 => eggfetch_core::HttpVersionPolicy::Http1Only,
        1 => eggfetch_core::HttpVersionPolicy::Http2Only,
        2 => eggfetch_core::HttpVersionPolicy::Auto { allow_http3: false },
        3 => eggfetch_core::HttpVersionPolicy::Auto { allow_http3: true },
        _ => return -1,
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.http_version_policy(version_policy));
    0
}

/// Enable or disable automatic decompression.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_automatic_decompression(
    handle: *mut ClientBuilderHandle,
    enabled: i32,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.automatic_decompression(enabled != 0));
    0
}

/// Set the maximum decoded body size in bytes.
///
/// When set, responses whose decompressed body exceeds this limit
/// produce an error. Do not call this function to use the default (no limit).
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_max_decoded_body_size(
    handle: *mut ClientBuilderHandle,
    max: usize,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.max_decoded_body_size(max));
    0
}

/// Set the maximum decompression ratio.
///
/// Responses whose expansion ratio exceeds this limit produce an error.
/// This guards against zip-bomb style attacks.
/// Do not call this function to use the default (no limit).
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_max_decompression_ratio(
    handle: *mut ClientBuilderHandle,
    ratio: f64,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.max_decompression_ratio(ratio));
    0
}

/// Set the maximum number of idle connections in the pool.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_max_idle_connections(
    handle: *mut ClientBuilderHandle,
    max: usize,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.max_idle_connections(max));
    0
}

/// Set the maximum number of idle connections per host.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_max_idle_connections_per_host(
    handle: *mut ClientBuilderHandle,
    max: usize,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.max_idle_connections_per_host(max));
    0
}

/// Enable or disable insecure TLS (skip certificate verification).
///
/// **WARNING**: This disables certificate verification and should only be
/// used for testing. Never use this in production.
///
/// Returns 0 on success, -1 if handle is invalid.
///
/// # Safety
///
/// `handle` must be a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_danger_accept_invalid_certs(
    handle: *mut ClientBuilderHandle,
    accept: i32,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let tls = eggfetch_core::TlsConfig::builder()
        .danger_accept_invalid_certs(accept != 0)
        .build();
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.tls_config(tls));
    0
}

/// Set a basic auth credential on the client.
///
/// All requests made with this client will include the Authorization header.
///
/// Returns 0 on success, -1 if handle or arguments are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed builder handle.
/// - `username` and `password` must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_basic_auth(
    handle: *mut ClientBuilderHandle,
    username: *const std::os::raw::c_char,
    password: *const std::os::raw::c_char,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let Some(username_str) = crate::handle::cstr_to_opt(username) else {
        return -1;
    };
    let Some(password_str) = crate::handle::cstr_to_opt(password) else {
        return -1;
    };
    let Ok(auth) = eggfetch_core::AuthScheme::basic(username_str, password_str) else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.auth(auth));
    0
}

/// Set a bearer auth token on the client.
///
/// All requests made with this client will include the Authorization header.
///
/// Returns 0 on success, -1 if handle or arguments are invalid.
///
/// # Safety
///
/// - `handle` must be a valid, non-freed builder handle.
/// - `token` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_bearer_auth(
    handle: *mut ClientBuilderHandle,
    token: *const std::os::raw::c_char,
) -> i32 {
    let Some(handle) = handle.as_mut() else {
        return -1;
    };
    let Some(token_str) = crate::handle::cstr_to_opt(token) else {
        return -1;
    };
    let Ok(auth) = eggfetch_core::AuthScheme::bearer(token_str) else {
        return -1;
    };
    let builder = std::ptr::read(&handle.0);
    std::ptr::write(&mut handle.0, builder.auth(auth));
    0
}
