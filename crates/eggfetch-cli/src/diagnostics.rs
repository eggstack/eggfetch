//! Optional diagnostics channel for the CLI.
//!
//! The engine emits `tracing` events for conditions an operator should be
//! able to see — a dropped non-UTF-8 header, a skipped malformed cookie, a
//! trust-store substitution. The engine is a library, so it deliberately
//! leaves the subscriber to the embedder. The CLI is a binary: with no
//! subscriber installed, those events are discarded and the operator sees a
//! silent absence instead of a signal.
//!
//! This module installs the smallest subscriber that surfaces them. Engine
//! diagnostics go to stderr, one line each, with credentials redacted the
//! same way the CLI redacts its own error output.
//!
//! The feature is opt-in and off by default, so the published binary keeps
//! its current behaviour and its current dependency set.

use std::fmt;

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

/// Collects the `message` field of an event.
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            value.clone_into(&mut self.message);
        }
    }
}

/// A subscriber that writes engine diagnostics to stderr.
pub(crate) struct StderrSubscriber;

impl Subscriber for StderrSubscriber {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        // The engine does not create spans, so there is no span state to
        // track. Every span gets the same sentinel id.
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        if visitor.message.is_empty() {
            return;
        }
        eprintln!(
            "[eggfetch:{}] {}",
            event.metadata().level(),
            crate::output::redact_credentials(&visitor.message)
        );
    }

    fn enter(&self, _id: &Id) {}

    fn exit(&self, _id: &Id) {}
}

/// Install the diagnostics subscriber for this process.
///
/// Returns `true` when this call installed the subscriber. A `false` result
/// means another subscriber was already in place — for example a second
/// installation in the same process — and the existing one is kept.
pub(crate) fn install() -> bool {
    tracing::subscriber::set_global_default(StderrSubscriber).is_ok()
}
