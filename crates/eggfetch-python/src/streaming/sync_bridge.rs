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
pub(super) struct SyncBridge<T> {
    state: Mutex<SyncQueueState<T>>,
    available: Condvar,
    space: tokio::sync::Notify,
    capacity: usize,
}

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
                let mut state = self.state.lock().expect("sync stream bridge poisoned");
                if state.closed {
                    return false;
                }
                if state.queue.len() < self.capacity {
                    state.queue.push_back(item.take().expect("item present"));
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
        let mut state = self.state.lock().expect("sync stream bridge poisoned");
        loop {
            if let Some(item) = state.queue.pop_front() {
                self.space.notify_one();
                return Some(item);
            }
            if state.closed {
                return None;
            }
            state = self
                .available
                .wait(state)
                .expect("sync stream bridge poisoned");
        }
    }

    pub(super) fn close(&self) {
        let mut state = self.state.lock().expect("sync stream bridge poisoned");
        state.closed = true;
        self.available.notify_all();
        self.space.notify_waiters();
    }
}

// ---------------------------------------------------------------------------
// PyStreamingResponse
// ---------------------------------------------------------------------------
