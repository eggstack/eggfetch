//! TLS configuration helpers for Python bindings.

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyTuple};

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
    } else if let Ok(py_list) = v.cast::<PyList>() {
        let mut der_certs: Vec<Vec<u8>> = Vec::with_capacity(py_list.len());
        for item in py_list.iter() {
            if let Ok(der_bytes) = item.extract::<Vec<u8>>() {
                der_certs.push(der_bytes);
            } else if let Ok(py_bytes) = item.cast::<PyBytes>() {
                der_certs.push(py_bytes.as_bytes().to_vec());
            } else {
                return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                    "verify list must contain bytes objects (DER certificates)",
                ));
            }
        }
        builder = builder.ca_certificate_der(der_certs).map_err(map_err)?;
    } else {
        // Anything that is not an ssl.SSLContext (or subclass) is
        // unsupported; fail with an explicit TypeError instead of
        // falling into the snapshot helper's import machinery.
        let ssl_context_cls = v.py().import("ssl")?.getattr("SSLContext")?;
        if !v.is_instance(&ssl_context_cls)? {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "verify must be bool, a CA bundle path, \
                 a list of DER certificates, or an ssl.SSLContext",
            ));
        }
        // Extract snapshot via Python-side helper.
        builder = apply_ssl_context(builder, v)?;
    }
    Ok(builder)
}

/// Apply an ssl.SSLContext verify value to the TLS config builder.
fn apply_ssl_context(
    builder: eggfetch_core::TlsConfigBuilder,
    v: &Bound<'_, PyAny>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    let snapshot_mod = v.py().import("eggfetch._ssl_context")?;
    let export = snapshot_mod
        .getattr("_export_ssl_context_state")?
        .call1((v.as_unbound(),))?;
    let payload = export.cast::<PyDict>().map_err(|_| {
        PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "invalid SSLContext export: expected a mapping",
        )
    })?;
    let schema_version: i32 = required_export_item(payload, "schema_version")?
        .extract()
        .map_err(|_| export_type_error("schema_version"))?;
    if schema_version != 1 {
        return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "unsupported SSLContext export schema version: {schema_version}"
        )));
    }
    let class_str: String = required_export_item(payload, "classification")?
        .extract()
        .map_err(|_| export_type_error("classification"))?;

    if class_str == "unrepresentable" {
        return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "eggfetch cannot safely translate this ssl.SSLContext; \
             use eggfetch.compat.httpx.create_ssl_context() or pass \
             verify/cert kwargs directly",
        ));
    }

    if class_str != "exactly_representable" && class_str != "representable_with_known_defaults" {
        return Err(export_type_error("classification"));
    }

    let metadata = required_export_item(payload, "helper_metadata")?;
    if metadata.is_none() {
        apply_export_to_builder(builder, payload)
    } else {
        apply_helper_metadata(
            builder,
            metadata
                .cast::<PyDict>()
                .map_err(|_| export_type_error("helper_metadata"))?,
        )
    }
}

fn export_type_error(field: &str) -> PyErr {
    PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
        "invalid SSLContext export field: {field}"
    ))
}

fn required_export_item<'py>(
    payload: &Bound<'py, PyDict>,
    field: &str,
) -> PyResult<Bound<'py, PyAny>> {
    payload
        .get_item(field)?
        .ok_or_else(|| export_type_error(field))
}

fn apply_helper_metadata(
    mut builder: eggfetch_core::TlsConfigBuilder,
    metadata: &Bound<'_, PyDict>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    let verify = required_export_item(metadata, "verify")?;
    if let Ok(value) = verify.extract::<bool>() {
        if !value {
            builder = builder.danger_accept_invalid_certs(true);
        }
    } else if let Ok(path) = verify.extract::<String>() {
        builder = builder.ca_certificate_path(&path).map_err(map_err)?;
    } else {
        return Err(export_type_error("helper_metadata.verify"));
    }

    let cert_path: Option<String> = required_export_item(metadata, "cert_path")?
        .extract()
        .map_err(|_| export_type_error("helper_metadata.cert_path"))?;
    let key_path: Option<String> = required_export_item(metadata, "key_path")?
        .extract()
        .map_err(|_| export_type_error("helper_metadata.key_path"))?;
    if let Some(cert_path) = cert_path {
        let key_path = key_path.unwrap_or_else(|| cert_path.clone());
        builder = builder
            .client_cert_path(&cert_path, &key_path)
            .map_err(map_err)?;
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
fn apply_export_to_builder(
    mut builder: eggfetch_core::TlsConfigBuilder,
    payload: &Bound<'_, PyDict>,
) -> PyResult<eggfetch_core::TlsConfigBuilder> {
    let verify_mode: i32 = required_export_item(payload, "verify_mode")?
        .extract()
        .map_err(|_| export_type_error("verify_mode"))?;
    let check_hostname: bool = required_export_item(payload, "check_hostname")?
        .extract()
        .map_err(|_| export_type_error("check_hostname"))?;
    let ca_der: Vec<Vec<u8>> = required_export_item(payload, "ca_certs_der")?
        .extract()
        .map_err(|_| export_type_error("ca_certs_der"))?;

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
    let min_version: Option<i32> = required_export_item(payload, "min_version")?
        .extract()
        .map_err(|_| export_type_error("min_version"))?;
    let max_version: Option<i32> = required_export_item(payload, "max_version")?
        .extract()
        .map_err(|_| export_type_error("max_version"))?;

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
    }
    if let Ok(capath) = std::env::var("SSL_CERT_DIR") {
        if !capath.is_empty() {
            builder = builder.add_ca_certificate_path(&capath).map_err(map_err)?;
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
    } else if let Ok(tuple) = c.cast::<PyTuple>() {
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
