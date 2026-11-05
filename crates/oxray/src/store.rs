//! Bounded in-memory capture store.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::capture::Capture;

#[derive(Clone, Debug)]
struct Stored {
    capture: Capture,
    bytes: usize,
}

/// Ring of captures capped by count and encoded size.
#[derive(Debug)]
pub struct MemoryStore {
    inner: Mutex<State>,
    max_captures: usize,
    max_bytes: usize,
}

#[derive(Debug)]
struct State {
    items: VecDeque<Stored>,
    bytes: usize,
}

/// Usage numbers for `/api/meta`.
#[derive(Clone, Copy, Debug)]
pub struct StoreUsage {
    /// Captures currently stored.
    pub captures: usize,
    /// Encoded bytes currently stored.
    pub bytes: usize,
    /// Configured capture cap.
    pub max_captures: usize,
    /// Configured byte cap.
    pub max_bytes: usize,
}

impl MemoryStore {
    /// Empty store with the given caps.
    pub fn new(max_captures: usize, max_bytes: usize) -> Self {
        Self {
            inner: Mutex::new(State {
                items: VecDeque::new(),
                bytes: 0,
            }),
            max_captures,
            max_bytes,
        }
    }

    /// Inserts `capture`, evicting the oldest until both caps hold.
    pub fn insert(&self, capture: Capture) {
        let size = serde_json::to_vec(&capture)
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        if size > self.max_bytes {
            return;
        }
        let mut state = lock(&self.inner);
        while state.items.len() >= self.max_captures || state.bytes + size > self.max_bytes {
            let Some(old) = state.items.pop_front() else {
                break;
            };
            state.bytes = state.bytes.saturating_sub(old.bytes);
        }
        if state.bytes + size > self.max_bytes {
            return;
        }
        state.bytes += size;
        state.items.push_back(Stored {
            capture,
            bytes: size,
        });
    }

    /// Capture with this id.
    pub fn get(&self, id: &str) -> Option<Capture> {
        lock(&self.inner)
            .items
            .iter()
            .find(|item| item.capture.id == id)
            .map(|item| item.capture.clone())
    }

    /// Newest-first page.
    pub fn page(&self, cursor: Option<&str>, limit: usize) -> (Vec<Capture>, Option<String>) {
        let state = lock(&self.inner);
        let mut ids: Vec<Capture> = state
            .items
            .iter()
            .map(|item| item.capture.clone())
            .collect();
        ids.sort_by(|left, right| right.id.cmp(&left.id));
        if let Some(cursor) = cursor {
            ids.retain(|capture| capture.id.as_str() < cursor);
        }
        let limit = limit.clamp(1, 500);
        let has_more = ids.len() > limit;
        ids.truncate(limit);
        let next = if has_more {
            ids.last().map(|capture| capture.id.clone())
        } else {
            None
        };
        (ids, next)
    }

    /// Current usage.
    pub fn usage(&self) -> StoreUsage {
        let state = lock(&self.inner);
        StoreUsage {
            captures: state.items.len(),
            bytes: state.bytes,
            max_captures: self.max_captures,
            max_bytes: self.max_bytes,
        }
    }
}

fn lock(mutex: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
