//! Core trace observer abstraction for HTTPX-compatible request tracing.
//!
//! This module defines a typed event vocabulary derived from httpcore 1.0.9,
//! plus a callback trait that transports use to emit lifecycle events.
//! The Python binding implements this trait and bridges to user-supplied
//! trace callables without holding the GIL during network waits.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

/// Typed lifecycle events emitted by the transport layer.
///
/// Event names and phases match httpcore 1.0.9's `Trace` context manager
/// vocabulary. Each event has a phase (`Started`, `Complete`, `Failed`)
/// and carries structured metadata as a map of string keys to opaque values.
///
/// The Python compatibility layer maps these to the dotted event names
/// that HTTPX's `trace` extension expects (e.g. `connect_tcp.started`).
#[derive(Debug, Clone)]
pub enum TraceEvent {
    /// TCP connection to the target host.
    ConnectTcp {
        /// Lifecycle phase.
        phase: TracePhase,
        /// Target hostname or IP address.
        host: String,
        /// Target port number.
        port: u16,
    },
    /// Unix domain socket connection.
    ConnectUnixSocket {
        /// Lifecycle phase.
        phase: TracePhase,
        /// Path to the Unix domain socket.
        path: String,
    },
    /// TLS handshake (client hello through finished).
    StartTls {
        /// Lifecycle phase.
        phase: TracePhase,
        /// Hostname used for TLS Server Name Indication.
        server_hostname: String,
    },
    /// Connection retry with backoff delay.
    Retry {
        /// Lifecycle phase.
        phase: TracePhase,
        /// Delay in milliseconds before the retry attempt.
        delay_ms: u64,
    },
    /// Connection closed.
    Close {
        /// Lifecycle phase.
        phase: TracePhase,
    },
    /// HTTP/1.1 or HTTP/2 request headers sent.
    SendRequestHeaders {
        /// Lifecycle phase.
        phase: TracePhase,
        /// HTTP method (e.g. `"GET"`).
        method: String,
        /// Request target (e.g. `"/path"`).
        target: String,
    },
    /// HTTP/1.1 or HTTP/2 request body chunks sent.
    SendRequestBody {
        /// Lifecycle phase.
        phase: TracePhase,
    },
    /// HTTP/1.1 or HTTP/2 response status/headers received.
    ReceiveResponseHeaders {
        /// Lifecycle phase.
        phase: TracePhase,
        /// HTTP status code.
        status: u16,
    },
    /// HTTP/1.1 or HTTP/2 response body chunks received.
    ReceiveResponseBody {
        /// Lifecycle phase.
        phase: TracePhase,
    },
    /// Response stream closed; connection returned to pool or dropped.
    ResponseClosed {
        /// Lifecycle phase.
        phase: TracePhase,
    },
}

/// Lifecycle phase of a trace event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TracePhase {
    /// Operation started.
    Started,
    /// Operation completed successfully.
    Complete,
    /// Operation failed with an error.
    Failed,
}

impl fmt::Display for TracePhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Started => write!(f, "started"),
            Self::Complete => write!(f, "complete"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Map a [`TraceEvent`] to the httpcore 1.0.9 dotted event name.
///
/// Returns `(prefix, event_name)` where `prefix` is the httpcore
/// logger name suffix (e.g. `"http11"`, `"connection"`) and
/// `event_name` is the full dotted name (e.g. `"send_request_headers.started"`).
pub fn event_to_httpcore_name(event: &TraceEvent) -> (&'static str, String) {
    match event {
        TraceEvent::ConnectTcp { phase, .. } => ("connection", format!("connect_tcp.{phase}")),
        TraceEvent::ConnectUnixSocket { phase, .. } => {
            ("connection", format!("connect_unix_socket.{phase}"))
        }
        TraceEvent::StartTls { phase, .. } => ("connection", format!("start_tls.{phase}")),
        TraceEvent::Retry { phase, .. } => ("connection", format!("retry.{phase}")),
        TraceEvent::Close { phase } => ("connection", format!("close.{phase}")),
        TraceEvent::SendRequestHeaders { phase, .. } => {
            ("http11", format!("send_request_headers.{phase}"))
        }
        TraceEvent::SendRequestBody { phase } => ("http11", format!("send_request_body.{phase}")),
        TraceEvent::ReceiveResponseHeaders { phase, .. } => {
            ("http11", format!("receive_response_headers.{phase}"))
        }
        TraceEvent::ReceiveResponseBody { phase } => {
            ("http11", format!("receive_response_body.{phase}"))
        }
        TraceEvent::ResponseClosed { phase } => ("http11", format!("response_closed.{phase}")),
    }
}

/// Convert a [`TraceEvent`] into the flat dictionary representation
/// that httpcore's trace callback receives.
pub fn event_to_info(event: &TraceEvent) -> HashMap<String, EventValue> {
    let mut info = HashMap::new();
    match event {
        TraceEvent::ConnectTcp { host, port, .. } => {
            info.insert("host".into(), EventValue::String(host.clone()));
            info.insert("port".into(), EventValue::U16(*port));
        }
        TraceEvent::ConnectUnixSocket { path, .. } => {
            info.insert("path".into(), EventValue::String(path.clone()));
        }
        TraceEvent::StartTls {
            server_hostname, ..
        } => {
            info.insert(
                "server_hostname".into(),
                EventValue::String(server_hostname.clone()),
            );
        }
        TraceEvent::Retry { delay_ms, .. } => {
            info.insert("delay_ms".into(), EventValue::U64(*delay_ms));
        }
        TraceEvent::SendRequestHeaders { method, target, .. } => {
            info.insert("method".into(), EventValue::String(method.clone()));
            info.insert("target".into(), EventValue::String(target.clone()));
        }
        TraceEvent::ReceiveResponseHeaders { status, .. } => {
            info.insert("status".into(), EventValue::U16(*status));
        }
        _ => {}
    }
    info
}

/// Opaque value type for trace event info dictionaries.
#[derive(Debug, Clone, PartialEq)]
pub enum EventValue {
    /// A string value.
    String(String),
    /// A u16 value (e.g. status code, port).
    U16(u16),
    /// A u64 value (e.g. delay in milliseconds).
    U64(u64),
}

/// Callback trait for receiving trace events from the transport layer.
///
/// Implementations must be `Send + Sync` because trace observers are
/// stored on requests that may be sent across tokio tasks. The callback
/// is invoked synchronously within the transport's async context; it
/// must not block or perform I/O.
///
/// For Python bindings, the implementation acquires the GIL only at
/// callback delivery points, never across network waits.
pub trait TraceObserver: Send + Sync + fmt::Debug {
    /// Called when a trace event occurs.
    fn on_event(&self, event: &TraceEvent);
}

/// A no-op trace observer that silently discards all events.
#[derive(Debug)]
pub struct NoopTraceObserver;

impl TraceObserver for NoopTraceObserver {
    fn on_event(&self, _event: &TraceEvent) {}
}

/// A trace observer that collects events into a shared vector.
///
/// Primarily used for testing and debugging.
#[derive(Debug, Clone)]
pub struct CollectingTraceObserver {
    events: Arc<std::sync::Mutex<Vec<TraceEvent>>>,
}

impl Default for CollectingTraceObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectingTraceObserver {
    /// Create a new collecting observer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Drain and return all collected events.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn drain(&self) -> Vec<TraceEvent> {
        std::mem::take(&mut *self.events.lock().expect("trace collector mutex poisoned"))
    }

    /// Return the number of collected events without consuming them.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events
            .lock()
            .expect("trace collector mutex poisoned")
            .len()
    }

    /// Returns `true` if no events have been collected.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl TraceObserver for CollectingTraceObserver {
    fn on_event(&self, event: &TraceEvent) {
        self.events
            .lock()
            .expect("trace collector mutex poisoned")
            .push(event.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_to_httpcore_name_connect_tcp() {
        let event = TraceEvent::ConnectTcp {
            phase: TracePhase::Started,
            host: "example.com".into(),
            port: 443,
        };
        let (prefix, name) = event_to_httpcore_name(&event);
        assert_eq!(prefix, "connection");
        assert_eq!(name, "connect_tcp.started");
    }

    #[test]
    fn event_to_httpcore_name_send_headers() {
        let event = TraceEvent::SendRequestHeaders {
            phase: TracePhase::Complete,
            method: "GET".into(),
            target: "/".into(),
        };
        let (prefix, name) = event_to_httpcore_name(&event);
        assert_eq!(prefix, "http11");
        assert_eq!(name, "send_request_headers.complete");
    }

    #[test]
    fn event_to_info_connect_tcp() {
        let event = TraceEvent::ConnectTcp {
            phase: TracePhase::Started,
            host: "example.com".into(),
            port: 443,
        };
        let info = event_to_info(&event);
        assert_eq!(
            info.get("host"),
            Some(&EventValue::String("example.com".into()))
        );
        assert_eq!(info.get("port"), Some(&EventValue::U16(443)));
    }

    #[test]
    fn collecting_observer_stores_events() {
        let observer = CollectingTraceObserver::new();
        observer.on_event(&TraceEvent::Close {
            phase: TracePhase::Started,
        });
        observer.on_event(&TraceEvent::Close {
            phase: TracePhase::Complete,
        });
        assert_eq!(observer.len(), 2);
        let events = observer.drain();
        assert_eq!(events.len(), 2);
        assert!(observer.is_empty());
    }
}
