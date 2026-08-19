//! TLS configuration helpers for Python bindings.

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyList, PyTuple};

use crate::errors::map_err;

/// Apply verify kwargs to the TLS config builder.
fn apply_verify(
    mut builder: eggfetch_core::TlsConfigBuilder,
    v: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    if let Ok(b) = v.extract::<bool>() {
        if !b {
            builder = builder.danger_accept_invalid_certs(true);
        }
    } else if let Ok(path) = v.extract::<String>() {
        builder = builder.ca_certificate_path(&path).map_err(map_err)?;
    } else if let Ok(py_list) = v.downcast::<PyList>() {
        let mut der_certs: Vec<Vec<u8>> = Vec::with_capacity(py_list.len());
        for item in py_list.iter() {
            if let Ok(der_bytes) = item.extract::<Vec<u8>>() {
                der_certs.push(der_bytes);
            } else if let Ok(py_bytes) = item.downcast::<PyBytes>() {
                der_certs.push(py_bytes.as_bytes().to_vec());
            } else {
                return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                    "verify list must contain bytes objects (DER certificates)",
                ));
            }
        }
        builder = builder.ca_certificate_der(der_certs).map_err(map_err)?;
    } else {
        // ssl.SSLContext or other object: extract snapshot via Python-side helper.
        builder = apply_ssl_context(builder, v)?;
    }
    Ok(builder)
}

/// Apply an ssl.SSLContext verify value to the TLS config builder.
fn apply_ssl_context(
    mut builder: eggfetch_core::TlsConfigBuilder,
    v: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    let snapshot_mod = v.py().import("eggfetch.compat.httpx._ssl_context")?;
    let snapshot = snapshot_mod
        .getattr("snapshot_context")?
        .call((v.as_unbound(),), None)?;
    let classification = snapshot_mod
        .getattr("_classify_context")?
        .call((v.as_unbound(), &snapshot), None)?;
    let class_str: String = classification.extract()?;

    if class_str == "unrepresentable" {
        return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "eggfetch cannot safely translate this ssl.SSLContext; \
             use eggfetch.compat.httpx.create_ssl_context() or pass \
             verify/cert kwargs directly",
        ));
    }

    let registry = snapshot_mod.getattr("_eggfetch_ssl_registry")?;
    let is_eggfetch: bool = registry
        .getattr("is_eggfetch_context")?
        .call((v.as_unbound(),), None)?
        .extract()?;

    if is_eggfetch {
        builder = apply_eggfetch_registry_metadata(builder, &registry, v)?;
    } else {
        builder = apply_snapshot_to_builder(builder, &snapshot)?;
    }

    Ok(builder)
}

/// Apply metadata from the eggfetch SSL registry to the builder.
fn apply_eggfetch_registry_metadata(
    mut builder: eggfetch_core::TlsConfigBuilder,
    registry: &Bound<'_, PyAny>,
    ctx: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    let meta_any = registry.getattr("get")?.call((ctx.as_unbound(),), None)?;

    if !meta_any.is_none() {
        let meta = meta_any.downcast::<pyo3::types::PyDict>()?;

        if let Some(verify_val) = meta.get_item("verify")? {
            if let Ok(b) = verify_val.extract::<bool>() {
                if !b {
                    builder = builder.danger_accept_invalid_certs(true);
                }
            } else if let Ok(path) = verify_val.extract::<String>() {
                builder = builder.ca_certificate_path(&path).map_err(map_err)?;
            }
        }
        if let Some(cert_path_val) = meta.get_item("cert_path")? {
            let cp: String = cert_path_val.extract()?;
            let kp: String = meta
                .get_item("key_path")?
                .and_then(|v| v.extract::<String>().ok())
                .unwrap_or_else(|| cp.clone());
            builder = builder.client_cert_path(&cp, &kp).map_err(map_err)?;
        }
    }

    Ok(builder)
}

/// Python `ssl.TLSVersion` wire values: `TLSv1_2=771`, `TLSv1_3=772`.
const TLS_1_2_WIRE: i32 = 771;
const TLS_1_3_WIRE: i32 = 772;
/// Default sentinels when `SSLContext` has not set min/max version.
const DEFAULT_MIN_SENTINEL: i32 = -2;
const DEFAULT_MAX_SENTINEL: i32 = -1;

/// Apply snapshot values to the TLS config builder.
fn apply_snapshot_to_builder(
    mut builder: eggfetch_core::TlsConfigBuilder,
    snapshot: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    let verify_mode: i32 = snapshot.getattr("verify_mode")?.extract()?;
    let check_hostname: bool = snapshot.getattr("check_hostname")?.extract()?;
    let ca_der: Vec<Vec<u8>> = snapshot.getattr("ca_certs_der")?.extract()?;

    if verify_mode == 0 {
        // ssl.CERT_NONE
        builder = builder.danger_accept_invalid_certs(true);
    } else if !ca_der.is_empty() {
        builder = builder.ca_certificate_der(ca_der).map_err(map_err)?;
    }

    if !check_hostname {
        builder = builder.verify_hostname(false);
    }

    // Apply TLS version bounds from the snapshot.
    let min_version: Option<i32> = snapshot.getattr("min_version")?.extract()?;
    let max_version: Option<i32> = snapshot.getattr("max_version")?.extract()?;

    if let Some(v) = min_version {
        if v > 0 && v != DEFAULT_MIN_SENTINEL {
            let tls_version = match v {
                TLS_1_2_WIRE => eggfetch_core::TlsVersion::Tls12,
                TLS_1_3_WIRE => eggfetch_core::TlsVersion::Tls13,
                _ => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "unsupported TLS minimum version: {v}"
                    )));
                }
            };
            builder = builder.min_tls_version(tls_version);
        }
    }

    if let Some(v) = max_version {
        if v > 0 && v != DEFAULT_MAX_SENTINEL {
            let tls_version = match v {
                TLS_1_2_WIRE => eggfetch_core::TlsVersion::Tls12,
                TLS_1_3_WIRE => eggfetch_core::TlsVersion::Tls13,
                _ => {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "unsupported TLS maximum version: {v}"
                    )));
                }
            };
            builder = builder.max_tls_version(tls_version);
        }
    }

    Ok(builder)
}

/// Apply `trust_env` to load CA from environment variables.
fn apply_trust_env(
    mut builder: eggfetch_core::TlsConfigBuilder,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    if let Ok(cafile) = std::env::var("SSL_CERT_FILE") {
        if !cafile.is_empty() {
            builder = builder.ca_certificate_path(&cafile).map_err(map_err)?;
        }
    } else if let Ok(capath) = std::env::var("SSL_CERT_DIR") {
        if !capath.is_empty() {
            builder = builder.ca_certificate_path(&capath).map_err(map_err)?;
        }
    }
    Ok(builder)
}

/// Apply client certificate to the TLS config builder.
fn apply_cert(
    mut builder: eggfetch_core::TlsConfigBuilder,
    c: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    if let Ok(path) = c.extract::<String>() {
        builder = builder.client_cert_path(&path, &path).map_err(map_err)?;
    } else if let Ok(tuple) = c.downcast::<PyTuple>() {
        if tuple.len() != 2 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "cert tuple must be (cert_path, key_path)",
            ));
        }
        let cert_path: String = tuple.get_item(0)?.extract()?;
        let key_path: String = tuple.get_item(1)?.extract()?;
        builder = builder
            .client_cert_path(&cert_path, &key_path)
            .map_err(map_err)?;
    } else {
        return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "cert must be a str or tuple(cert_path, key_path)",
        ));
    }
    Ok(builder)
}

/// Build a `TlsConfig` from Python `verify`, `cert`, and `trust_env` kwargs.
///
/// - `verify`: `None`/`True` (default), `False` (skip verification),
///   `str` (CA bundle path), `ssl.SSLContext` (snapshot-translated), or
///   `list[bytes]` (DER-encoded CA certificates).
/// - `cert`: `None` (default), `str` (combined PEM), or `tuple(str, str)` (cert, key).
/// - `trust_env`: `bool` (default `True`), honour `SSL_CERT_FILE`/`SSL_CERT_DIR`.
pub fn build_tls_config(
    verify: Option<&Bound<'_, PyAny>>,
    cert: Option<&Bound<'_, PyAny>>,
    trust_env: Option<bool>,
) -> PyResult<eggfetch_core::TlsConfig> {
    let mut builder = eggfetch_core::TlsConfig::builder();
    let trust_env = trust_env.unwrap_or(true);

    if let Some(v) = verify {
        builder = apply_verify(builder, v)?;
    } else if trust_env {
        builder = apply_trust_env(builder)?;
    }

    if let Some(c) = cert {
        builder = apply_cert(builder, c)?;
    }

    Ok(builder.build())
}

/// Translate a Python `ssl.SSLContext` into an `eggfetch_core::TlsConfig`.
///
/// Returns `None` if the context is `None` or cannot be represented.
/// Returns `Err` if the context is explicitly unrepresentable.
pub fn ssl_context_to_tls_config(
    _py: Python<'_>,
    ssl_context: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<eggfetch_core::TlsConfig>> {
    let ctx = match ssl_context {
        Some(c) if !c.is_none() => c,
        _ => return Ok(None),
    };
    let builder = eggfetch_core::TlsConfig::builder();
    let builder = apply_ssl_context(builder, ctx)?;
    Ok(Some(builder.build()))
}
