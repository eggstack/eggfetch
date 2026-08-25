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
    crate::ffi_guard!(ptr::null_mut(), {
        Box::into_raw(Box::new(ClientBuilderHandle(Some(Client::builder()))))
    })
}

/// Free a client builder handle.
///
/// # Safety
///
/// `handle` must have been returned by [`eggfetch_client_builder_new`] and not freed yet.
/// Passing a null pointer is a no-op.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_free(handle: *mut ClientBuilderHandle) {
    crate::ffi_guard!((), {
        if !handle.is_null() {
            drop(Box::from_raw(handle));
        }
    });
}

/// Build a client from the configured builder.
///
/// The builder handle is **not** freed by this call. Release it with
/// [`eggfetch_client_builder_free`] exactly once regardless of outcome;
/// freeing an already-built builder is a safe no-op on the inner state.
///
/// Returns null when:
/// - `raw` is NULL or the builder was already built (programmer error), or
/// - allocating the client handle fails (resource exhaustion).
///
/// Caller must free a successful result with [`crate::handle::eggfetch_client_free`].
///
/// # Safety
///
/// `raw` must be NULL or a valid, non-freed builder handle.
#[no_mangle]
pub unsafe extern "C" fn eggfetch_client_builder_build(
    raw: *mut ClientBuilderHandle,
) -> *mut crate::handle::ClientHandle {
    crate::ffi_guard!(ptr::null_mut(), {
        let Some(handle) = raw.as_mut() else {
            return ptr::null_mut();
        };
        let Some(builder) = handle.0.take() else {
            // Already built: leave the shell allocation in place so the
            // caller can still release it exactly once via
            // eggfetch_client_builder_free (no dangling handle).
            return ptr::null_mut();
        };
        let client = builder.build();
        Box::into_raw(Box::new(crate::handle::ClientHandle(client)))
    })
}

/// Set the overall request timeout in seconds.
///
/// This is the total time limit for a complete request-response cycle,
/// including connection, sending, and receiving.
///
/// Like the per-phase setters, this merges into the current timeout
/// configuration: only the `total` phase is set; connect/read/write/pool
/// values from earlier calls are preserved.
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let timeout = eggfetch_core::Timeout::builder()
            .total(std::time::Duration::from_secs(secs))
            .build();
        // Merge instead of replace so setting one phase does not clobber
        // phases configured by earlier calls.
        update_builder(handle, |builder| builder.merge_timeout(timeout))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let timeout = eggfetch_core::Timeout::builder()
            .connect(std::time::Duration::from_secs(secs))
            .build();
        // Merge instead of replace so setting one phase does not clobber
        // phases configured by earlier calls.
        update_builder(handle, |builder| builder.merge_timeout(timeout))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let timeout = eggfetch_core::Timeout::builder()
            .read(std::time::Duration::from_secs(secs))
            .build();
        // Merge instead of replace so setting one phase does not clobber
        // phases configured by earlier calls.
        update_builder(handle, |builder| builder.merge_timeout(timeout))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let timeout = eggfetch_core::Timeout::builder()
            .write(std::time::Duration::from_secs(secs))
            .build();
        // Merge instead of replace so setting one phase does not clobber
        // phases configured by earlier calls.
        update_builder(handle, |builder| builder.merge_timeout(timeout))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_builder(handle, |builder| builder.follow_redirects(follow != 0))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_builder(handle, |builder| builder.max_redirects(max))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(agent_str) = crate::handle::cstr_to_string(agent) else {
            return -1;
        };
        update_builder(handle, |builder| builder.user_agent(&agent_str))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(name_str) = crate::handle::cstr_to_string(name) else {
            return -1;
        };
        let Some(value_str) = crate::handle::cstr_to_string(value) else {
            return -1;
        };
        update_builder_result(handle, |builder| {
            builder.default_header(&name_str, &value_str)
        })
    })
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
    crate::ffi_guard!(-1, {
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
        update_builder(handle, |builder| {
            builder.http_version_policy(version_policy)
        })
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_builder(handle, |builder| {
            builder.automatic_decompression(enabled != 0)
        })
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_builder(handle, |builder| builder.max_decoded_body_size(max))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_builder(handle, |builder| builder.max_decompression_ratio(ratio))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_builder(handle, |builder| builder.max_idle_connections(max))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        update_builder(handle, |builder| builder.max_idle_connections_per_host(max))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let tls = eggfetch_core::TlsConfig::builder()
            .danger_accept_invalid_certs(accept != 0)
            .build();
        update_builder(handle, |builder| builder.tls_config(tls))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(username_str) = crate::handle::cstr_to_string(username) else {
            return -1;
        };
        let Some(password_str) = crate::handle::cstr_to_string(password) else {
            return -1;
        };
        let Ok(auth) = eggfetch_core::AuthScheme::basic(username_str, password_str) else {
            return -1;
        };
        update_builder(handle, |builder| builder.auth(auth))
    })
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
    crate::ffi_guard!(-1, {
        let Some(handle) = handle.as_mut() else {
            return -1;
        };
        let Some(token_str) = crate::handle::cstr_to_string(token) else {
            return -1;
        };
        let Ok(auth) = eggfetch_core::AuthScheme::bearer(token_str) else {
            return -1;
        };
        update_builder(handle, |builder| builder.auth(auth))
    })
}

fn update_builder<F>(handle: &mut ClientBuilderHandle, update: F) -> i32
where
    F: FnOnce(eggfetch_core::ClientBuilder) -> eggfetch_core::ClientBuilder,
{
    let Some(builder) = handle.0.take() else {
        return -1;
    };
    handle.0 = Some(update(builder));
    0
}

fn update_builder_result<F>(handle: &mut ClientBuilderHandle, update: F) -> i32
where
    F: FnOnce(eggfetch_core::ClientBuilder) -> eggfetch_core::Result<eggfetch_core::ClientBuilder>,
{
    let Some(builder) = handle.0.take() else {
        return -1;
    };
    match update(builder) {
        Ok(builder) => {
            handle.0 = Some(builder);
            0
        }
        Err(_) => -1,
    }
}
