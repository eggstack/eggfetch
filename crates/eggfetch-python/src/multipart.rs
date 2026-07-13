//! Python multipart/file-upload support.
//!
//! Provides `PyFile` (the `eggfetch.File` wrapper) and helpers to convert
//! Python `files=` kwargs into a Rust `Multipart` body.

use std::path::PathBuf;

use bytes::Bytes;
use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::conversion::python_headers_to_rust;
use crate::errors::map_err;
use eggfetch_core::multipart::{Multipart, Part, PartBody};

// ---------------------------------------------------------------------------
// PyFile — Python-visible File wrapper
// ---------------------------------------------------------------------------

/// A file path wrapper for multipart uploads.
///
/// Wraps a filesystem path so it can be passed as a value in the `files=`
/// kwarg. The file is read synchronously when the request is sent.
///
/// Args:
///     path: Filesystem path to the file.
///     filename: Override filename (default: basename of path).
///     `content_type`: Override content type (default: application/octet-stream).
#[pyclass(name = "File")]
#[derive(Debug, Clone)]
pub struct PyFile {
    path: PathBuf,
    filename: String,
    content_type: String,
}

#[pymethods]
impl PyFile {
    #[new]
    #[pyo3(signature = (path, *, filename=None, content_type=None))]
    fn new(
        path: &Bound<'_, PyAny>,
        filename: Option<&str>,
        content_type: Option<&str>,
    ) -> PyResult<Self> {
        let path_str: String = path.extract().map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "path must be a string or path-like object",
            )
        })?;
        let pb = PathBuf::from(&path_str);
        let default_filename = pb
            .file_name()
            .map_or_else(|| "file".to_owned(), |n| n.to_string_lossy().into_owned());
        Ok(Self {
            path: pb,
            filename: filename.unwrap_or(&default_filename).to_owned(),
            content_type: content_type
                .unwrap_or("application/octet-stream")
                .to_owned(),
        })
    }

    /// The file path.
    #[getter]
    fn path(&self) -> String {
        self.path.to_string_lossy().into_owned()
    }

    /// The filename to use in the multipart form.
    #[getter]
    fn filename(&self) -> &str {
        &self.filename
    }

    /// The content type for this file.
    #[getter]
    fn content_type(&self) -> &str {
        &self.content_type
    }

    fn __repr__(&self) -> String {
        format!(
            "File(path={:?}, filename={:?}, content_type={:?})",
            self.path.display(),
            self.filename,
            self.content_type
        )
    }
}

// ---------------------------------------------------------------------------
// Multipart body builder
// ---------------------------------------------------------------------------

/// Build a multipart body from Python `data` and `files` kwargs.
///
/// Returns `(request_body, content_type)` where `content_type` includes the
/// boundary parameter.
pub fn build_multipart_body<'py>(
    py: Python<'py>,
    data: Option<&Bound<'py, PyAny>>,
    files: &Bound<'py, PyAny>,
) -> PyResult<(eggfetch_core::RequestBody, String)> {
    let mut multipart = Multipart::new();
    let boundary = multipart.boundary().clone();

    if let Some(d) = data {
        add_form_fields(&mut multipart, py, d)?;
    }

    add_file_parts(&mut multipart, py, files)?;

    let ct = format!("multipart/form-data; boundary={boundary}");
    let body = multipart.into_body();
    Ok((body, ct))
}

/// Add form field parts from a dict or sequence of pairs.
fn add_form_fields(
    multipart: &mut Multipart,
    _py: Python<'_>,
    data: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let items = if data.get_type().hasattr("__getitem__")? && data.hasattr("items")? {
        data.call_method0("items")?
    } else {
        data.clone()
    };
    for item in items.try_iter()? {
        let item = item?;
        let tuple: Bound<'_, PyTuple> = item.downcast_into::<PyTuple>()?;
        if tuple.len() != 2 {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "expected a mapping or sequence of 2-tuples",
            ));
        }
        let key: String = tuple.get_item(0)?.extract()?;
        let value: String = tuple.get_item(1)?.extract()?;
        *multipart = std::mem::replace(multipart, Multipart::new())
            .text(&key, &value)
            .map_err(map_err)?;
    }
    Ok(())
}

/// Add file parts from the `files=` kwarg.
///
/// Accepts:
/// - A mapping of `field_name` -> `file_spec`
/// - A sequence of (`field_name`, `file_spec`) pairs
///
/// File spec variants:
/// - `bytes` or `str`: bare data with `field_name` as filename
/// - `(filename, data)`: filename + data
/// - `(filename, data, content_type)`: filename + data + content type
/// - `(filename, data, content_type, headers)`: full spec
/// - `File` object: path-based file
fn add_file_parts(
    multipart: &mut Multipart,
    py: Python<'_>,
    files: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let pairs = if files.get_type().hasattr("__getitem__")? && files.hasattr("items")? {
        let items = files.call_method0("items")?;
        let mut pairs = Vec::new();
        for item in items.try_iter()? {
            let item = item?;
            let tuple: Bound<'_, PyTuple> = item.downcast_into::<PyTuple>()?;
            let field_name: String = tuple.get_item(0)?.extract()?;
            let file_spec = tuple.get_item(1)?;
            pairs.push((field_name, file_spec));
        }
        pairs
    } else {
        let mut pairs = Vec::new();
        for item in files.try_iter()? {
            let item = item?;
            let tuple: Bound<'_, PyTuple> = item.downcast_into::<PyTuple>()?;
            if tuple.len() != 2 {
                return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                    "files sequence items must be 2-tuples of (field_name, file_spec)",
                ));
            }
            let field_name: String = tuple.get_item(0)?.extract()?;
            let file_spec = tuple.get_item(1)?;
            pairs.push((field_name, file_spec));
        }
        pairs
    };

    for (field_name, file_spec) in pairs {
        add_single_file_part(multipart, py, &field_name, &file_spec)?;
    }
    Ok(())
}

/// Add a single file part from a field name and file spec.
fn add_single_file_part(
    multipart: &mut Multipart,
    py: Python<'_>,
    field_name: &str,
    file_spec: &Bound<'_, PyAny>,
) -> PyResult<()> {
    if let Ok(py_file) = file_spec.extract::<PyFile>() {
        return add_path_file_part(multipart, field_name, &py_file);
    }

    if let Ok(b) = file_spec.extract::<Vec<u8>>() {
        let filename = field_name.to_owned();
        *multipart = std::mem::replace(multipart, Multipart::new())
            .bytes(
                field_name,
                &filename,
                "application/octet-stream",
                Bytes::from(b),
            )
            .map_err(map_err)?;
        return Ok(());
    }

    if let Ok(s) = file_spec.extract::<String>() {
        let filename = field_name.to_owned();
        *multipart = std::mem::replace(multipart, Multipart::new())
            .bytes(
                field_name,
                &filename,
                "text/plain",
                Bytes::from(s.into_bytes()),
            )
            .map_err(map_err)?;
        return Ok(());
    }

    if let Ok(tuple) = file_spec.downcast::<PyTuple>() {
        return add_tuple_file_part(multipart, py, field_name, tuple);
    }

    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(format!(
        "unsupported file spec type for field {field_name}: {}",
        file_spec.get_type().name()?
    )))
}

/// Add a file part from a tuple spec.
fn add_tuple_file_part(
    multipart: &mut Multipart,
    py: Python<'_>,
    field_name: &str,
    tuple: &Bound<'_, PyTuple>,
) -> PyResult<()> {
    let len = tuple.len();
    if !(2..=4).contains(&len) {
        return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
            "file tuple must be (filename, data), (filename, data, content_type), or (filename, data, content_type, headers)",
        ));
    }

    let filename: String = tuple.get_item(0)?.extract()?;
    let data_bytes = extract_data_bytes(&tuple.get_item(1)?)?;
    let content_type: String = if len >= 3 {
        tuple.get_item(2)?.extract()?
    } else {
        guess_content_type(&filename)
    };

    if len >= 4 {
        let headers_obj = tuple.get_item(3)?;
        if !headers_obj.is_none() {
            let headers = python_headers_to_rust(py, &headers_obj)?;
            let mut part =
                Part::new(field_name, PartBody::Bytes(data_bytes))
                    .filename(&filename)
                    .content_type(http::HeaderValue::from_str(&content_type).map_err(|e| {
                        PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string())
                    })?);
            for (name, value) in headers.iter() {
                part = part.header(name.as_str(), value.to_str().unwrap_or(""));
            }
            *multipart = std::mem::replace(multipart, Multipart::new())
                .part(part)
                .map_err(map_err)?;
            return Ok(());
        }
    }

    *multipart = std::mem::replace(multipart, Multipart::new())
        .bytes(field_name, &filename, &content_type, data_bytes)
        .map_err(map_err)?;
    Ok(())
}

/// Read a `File` wrapper into a `PartBody`.
fn add_path_file_part(
    multipart: &mut Multipart,
    field_name: &str,
    py_file: &PyFile,
) -> PyResult<()> {
    let data = std::fs::read(&py_file.path).map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyFileExistsError, _>(format!(
            "failed to read {}: {e}",
            py_file.path.display()
        ))
    })?;
    *multipart = std::mem::replace(multipart, Multipart::new())
        .bytes(
            field_name,
            &py_file.filename,
            &py_file.content_type,
            Bytes::from(data),
        )
        .map_err(map_err)?;
    Ok(())
}

/// Extract bytes from a Python bytes-like or string object.
fn extract_data_bytes(obj: &Bound<'_, PyAny>) -> PyResult<Bytes> {
    if let Ok(b) = obj.extract::<Vec<u8>>() {
        return Ok(Bytes::from(b));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(Bytes::from(s.into_bytes()));
    }
    Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
        "file data must be bytes or string",
    ))
}

/// Guess content type from filename extension.
fn guess_content_type(filename: &str) -> String {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "txt" => "text/plain".to_owned(),
        "html" | "htm" => "text/html".to_owned(),
        "json" => "application/json".to_owned(),
        "xml" => "application/xml".to_owned(),
        "pdf" => "application/pdf".to_owned(),
        "png" => "image/png".to_owned(),
        "jpg" | "jpeg" => "image/jpeg".to_owned(),
        "gif" => "image/gif".to_owned(),
        "svg" => "image/svg+xml".to_owned(),
        "zip" => "application/zip".to_owned(),
        _ => "application/octet-stream".to_owned(),
    }
}
