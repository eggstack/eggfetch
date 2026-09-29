//! Private bounded bridge for synchronous streaming.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};

struct SyncQueueState<T> {
    queue: VecDeque<T>,
    closed: bool,
}

/// Runtime-neutral bounded bridge for synchronous Python iterators.
///
/// Producers wait asynchronously for space; synchronous consumers wait on a
/// condition variable after releasing the GIL. This keeps backpressure
/// bounded without blocking a Tokio worker.
///
/// Note: capacity counts *messages*, not bytes. Bytes producers must split
/// oversized network chunks (see `MAX_BRIDGE_CHUNK_BYTES`) before `send()`
/// so a slow consumer cannot pin `capacity × unbounded-chunk` memory.
pub(super) struct SyncBridge<T> {
    state: Mutex<SyncQueueState<T>>,
    available: Condvar,
    space: tokio::sync::Notify,
    capacity: usize,
}

/// Maximum bytes per bridge message for bytes producers.
///
/// With capacity 16 this bounds a stalled consumer to ~1 MiB of queued
/// body bytes instead of `16 × unbounded-chunk`. Producers split larger
/// network chunks; consumers already reassemble via their pending buffer.
pub(super) const MAX_BRIDGE_CHUNK_BYTES: usize = 64 * 1024;

impl<T> SyncBridge<T> {
    pub(super) fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(SyncQueueState {
                queue: VecDeque::with_capacity(capacity),
                closed: false,
            }),
            available: Condvar::new(),
            space: tokio::sync::Notify::new(),
            capacity,
        })
    }

    pub(super) async fn send(&self, item: T) -> bool {
        let mut item = Some(item);
        loop {
            let notified = self.space.notified();
            let sent = {
                // Recover from poisoning (e.g. a callback raising while the
                // lock is held) instead of panicking across the PyO3 boundary.
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if state.closed {
                    return false;
                }
                if state.queue.len() < self.capacity {
                    let Some(value) = item.take() else {
                        // Item already consumed by a prior loop turn;
                        // treat as closed instead of panicking across
                        // the PyO3 boundary.
                        return false;
                    };
                    state.queue.push_back(value);
                    true
                } else {
                    false
                }
            };
            if sent {
                self.available.notify_one();
                return true;
            }
            notified.await;
        }
    }

    pub(super) fn recv_blocking(&self) -> Option<T> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if let Some(item) = state.queue.pop_front() {
                self.space.notify_one();
                return Some(item);
            }
            if state.closed {
                return None;
            }
            state = match self.available.wait(state) {
                Ok(state) => state,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }

    pub(super) fn close(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.closed = true;
        self.available.notify_all();
        self.space.notify_waiters();
    }
}

// ---------------------------------------------------------------------------
// PyStreamingResponse
// ---------------------------------------------------------------------------
