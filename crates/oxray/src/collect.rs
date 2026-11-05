//! Per-request collector and the bounded outgoing side table.

use std::collections::{HashMap, VecDeque};
#[cfg(feature = "reqwest")]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::capture::{
    BodyCapture, BodyEncoding, ErrorEntry, ErrorKind, Header, LogEntry, Message, NPlusOne,
    OutgoingCall, QueryEntry,
};
const MAX_LOGS: usize = 500;
const MAX_QUERIES: usize = 500;
const MAX_OUTGOING: usize = 100;
const MAX_SIDE: usize = 1_000;
const SIDE_TTL: Duration = Duration::from_secs(60);

/// Events gathered for one request.
pub(crate) struct Bundle {
    pub(crate) logs: Vec<LogEntry>,
    pub(crate) queries: Vec<QueryEntry>,
    pub(crate) outgoing: Vec<OutgoingCall>,
    pub(crate) errors: Vec<ErrorEntry>,
    pub(crate) n_plus_one: Vec<NPlusOne>,
    pub(crate) dropped_events: u64,
    pub(crate) saw_error: bool,
}

impl Bundle {
    pub(crate) fn empty() -> Self {
        Self {
            logs: Vec::new(),
            queries: Vec::new(),
            outgoing: Vec::new(),
            errors: Vec::new(),
            n_plus_one: Vec::new(),
            dropped_events: 0,
            saw_error: false,
        }
    }
}

pub(crate) struct OutgoingState {
    pub(crate) method: String,
    pub(crate) url: String,
    pub(crate) offset_us: u64,
    pub(crate) started: Instant,
    pub(crate) status: Option<u16>,
    pub(crate) request_headers: Vec<Header>,
    pub(crate) request_type: Option<String>,
    pub(crate) request_body: Option<String>,
    pub(crate) response_type: Option<String>,
    pub(crate) response_headers: Vec<Header>,
    pub(crate) response_body: Vec<u8>,
    pub(crate) response_truncated: bool,
    pub(crate) error: Option<String>,
    pub(crate) limit: usize,
    pub(crate) finished_us: Option<u64>,
}

impl OutgoingState {
    #[cfg(feature = "reqwest")]
    pub(crate) fn push_response(&mut self, chunk: &[u8]) {
        let room = self.limit.saturating_sub(self.response_body.len());
        if chunk.len() > room {
            self.response_body.extend_from_slice(&chunk[..room]);
            self.response_truncated = true;
        } else {
            self.response_body.extend_from_slice(chunk);
        }
    }
}

struct Collector {
    logs: Vec<LogEntry>,
    queries: Vec<QueryEntry>,
    outgoing: Vec<Arc<Mutex<OutgoingState>>>,
    dropped: u64,
    saw_error: bool,
}

/// Handle shared by the tower layer and the tracing layer.
#[derive(Clone)]
pub(crate) struct SharedCollector {
    inner: Arc<Mutex<Collector>>,
    started: Instant,
}

impl SharedCollector {
    fn new(started: Instant) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Collector::new())),
            started,
        }
    }

    pub(crate) fn offset(&self) -> u64 {
        offset_us(self.started)
    }
}

impl Collector {
    fn new() -> Self {
        Self {
            logs: Vec::new(),
            queries: Vec::new(),
            outgoing: Vec::new(),
            dropped: 0,
            saw_error: false,
        }
    }
}

struct SideSlot {
    at: Instant,
    data: Arc<Mutex<OutgoingState>>,
}

struct SideTable {
    order: VecDeque<u64>,
    map: HashMap<u64, SideSlot>,
}

impl SideTable {
    fn new() -> Self {
        Self {
            order: VecDeque::new(),
            map: HashMap::new(),
        }
    }

    fn purge(&mut self, now: Instant) {
        let expired: Vec<u64> = self
            .order
            .iter()
            .copied()
            .filter(|id| {
                self.map
                    .get(id)
                    .is_none_or(|slot| now.duration_since(slot.at) >= SIDE_TTL)
            })
            .collect();
        for id in expired {
            self.map.remove(&id);
            self.order.retain(|item| item != &id);
        }
        while self.map.len() >= MAX_SIDE {
            let Some(id) = self.order.pop_front() else {
                break;
            };
            self.map.remove(&id);
        }
    }

    #[cfg(feature = "reqwest")]
    fn insert(&mut self, id: u64, data: Arc<Mutex<OutgoingState>>) {
        let now = Instant::now();
        self.purge(now);
        self.order.push_back(id);
        self.map.insert(id, SideSlot { at: now, data });
    }

    fn take(&mut self, id: u64) -> Option<Arc<Mutex<OutgoingState>>> {
        self.purge(Instant::now());
        let slot = self.map.remove(&id)?;
        self.order.retain(|item| *item != id);
        Some(slot.data)
    }

    fn len(&mut self) -> usize {
        self.purge(Instant::now());
        self.map.len()
    }
}

/// Collectors and the outgoing side table shared by the layers.
pub(crate) struct Tables {
    collectors: Mutex<HashMap<String, SharedCollector>>,
    side: Mutex<SideTable>,
    #[cfg(feature = "reqwest")]
    next_side: AtomicU64,
    filter_warned: AtomicBool,
    n_plus_one_threshold: u64,
}

impl Tables {
    pub(crate) fn new(n_plus_one_threshold: u64) -> Self {
        Self {
            collectors: Mutex::new(HashMap::new()),
            side: Mutex::new(SideTable::new()),
            #[cfg(feature = "reqwest")]
            next_side: AtomicU64::new(1),
            filter_warned: AtomicBool::new(false),
            n_plus_one_threshold,
        }
    }

    pub(crate) fn begin(&self, id: &str, started: Instant) {
        let mut guard = lock_map(&self.collectors);
        guard.insert(id.to_owned(), SharedCollector::new(started));
    }

    pub(crate) fn collector(&self, id: &str) -> Option<SharedCollector> {
        lock_map(&self.collectors).get(id).cloned()
    }

    pub(crate) fn discard(&self, id: &str) {
        lock_map(&self.collectors).remove(id);
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        lock_map(&self.collectors).len()
    }

    pub(crate) fn finish(&self, id: &str, started: Instant) -> Bundle {
        let collector = lock_map(&self.collectors).remove(id);
        let Some(collector) = collector else {
            return Bundle::empty();
        };
        let collector = lock_collector_inner(&collector.inner);
        let _ = started;
        snapshot(&collector, self.n_plus_one_threshold)
    }

    #[cfg(feature = "reqwest")]
    pub(crate) fn park_outgoing(&self, data: Arc<Mutex<OutgoingState>>) -> u64 {
        let id = self.next_side.fetch_add(1, Ordering::Relaxed);
        let mut side = lock_side(&self.side);
        side.insert(id, data);
        id
    }

    pub(crate) fn take_outgoing(&self, id: u64) -> Option<Arc<Mutex<OutgoingState>>> {
        lock_side(&self.side).take(id)
    }

    pub(crate) fn pending_outgoing(&self) -> usize {
        lock_side(&self.side).len()
    }

    pub(crate) fn claim_filter_warn(&self) -> bool {
        !self.filter_warned.swap(true, Ordering::Relaxed)
    }
}

pub(crate) fn push_log(collector: &SharedCollector, entry: LogEntry, error: bool) {
    let mut guard = lock_collector_inner(&collector.inner);
    if error {
        guard.saw_error = true;
    }
    if guard.logs.len() >= MAX_LOGS {
        guard.dropped += 1;
        return;
    }
    guard.logs.push(entry);
}

pub(crate) fn push_query(collector: &SharedCollector, entry: QueryEntry) {
    let mut guard = lock_collector_inner(&collector.inner);
    if guard.queries.len() >= MAX_QUERIES {
        guard.dropped += 1;
        return;
    }
    guard.queries.push(entry);
}

pub(crate) fn push_outgoing(collector: &SharedCollector, call: Arc<Mutex<OutgoingState>>) {
    let mut guard = lock_collector_inner(&collector.inner);
    if guard.outgoing.len() >= MAX_OUTGOING {
        guard.dropped += 1;
        return;
    }
    guard.outgoing.push(call);
}

fn snapshot(collector: &Collector, threshold: u64) -> Bundle {
    let outgoing = collector.outgoing.iter().map(outgoing_call).collect();
    let n_plus_one = group_queries(&collector.queries, threshold);
    let errors = collector
        .logs
        .iter()
        .filter(|entry| entry.level == "ERROR")
        .map(|entry| ErrorEntry {
            offset_us: entry.offset_us,
            kind: ErrorKind::ErrorEvent,
            message: entry.message.clone(),
            target: Some(entry.target.clone()),
            location: None,
            backtrace: Vec::new(),
        })
        .collect();
    Bundle {
        logs: collector.logs.clone(),
        queries: collector.queries.clone(),
        outgoing,
        errors,
        n_plus_one,
        dropped_events: collector.dropped,
        saw_error: collector.saw_error,
    }
}

fn group_queries(queries: &[QueryEntry], threshold: u64) -> Vec<NPlusOne> {
    let mut order = Vec::new();
    let mut counts: HashMap<String, (u64, String)> = HashMap::new();
    for query in queries {
        if let Some(entry) = counts.get_mut(&query.fingerprint) {
            entry.0 += 1;
        } else {
            order.push(query.fingerprint.clone());
            counts.insert(query.fingerprint.clone(), (1, query.sql.clone()));
        }
    }
    order
        .into_iter()
        .filter_map(|fingerprint| {
            let (count, sql) = counts.remove(&fingerprint)?;
            if count >= threshold && threshold > 0 {
                Some(NPlusOne {
                    fingerprint,
                    count,
                    sql,
                })
            } else {
                None
            }
        })
        .collect()
}

fn outgoing_call(state: &Arc<Mutex<OutgoingState>>) -> OutgoingCall {
    let state = match state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let duration_us = state
        .finished_us
        .unwrap_or_else(|| u64::try_from(state.started.elapsed().as_micros()).unwrap_or(u64::MAX));
    let response = if state.status.is_some() || !state.response_headers.is_empty() {
        Some(Message {
            headers: state.response_headers.clone(),
            body: text_body(
                &state.response_body,
                state.response_truncated,
                state.limit,
                state.response_type.clone(),
            ),
        })
    } else {
        None
    };
    OutgoingCall {
        offset_us: state.offset_us,
        duration_us,
        method: state.method.clone(),
        url: state.url.clone(),
        status: state.status,
        request: Message {
            headers: state.request_headers.clone(),
            body: match &state.request_body {
                Some(text) => text_body(
                    text.as_bytes(),
                    false,
                    state.limit,
                    state.request_type.clone(),
                ),
                None => empty_body(),
            },
        },
        response,
        error: state.error.clone(),
    }
}

fn text_body(
    bytes: &[u8],
    truncated: bool,
    limit: usize,
    content_type: Option<String>,
) -> BodyCapture {
    let captured = bytes.len().min(limit);
    let slice = &bytes[..captured];
    match String::from_utf8(slice.to_vec()) {
        Ok(text) => BodyCapture {
            content_type: content_type.clone(),
            size_bytes: u64::try_from(bytes.len()).ok(),
            captured_bytes: u64::try_from(captured).unwrap_or(u64::MAX),
            encoding: BodyEncoding::Utf8,
            text: Some(text),
            truncated: truncated || captured < bytes.len(),
            omitted_reason: None,
        },
        Err(_) => BodyCapture {
            content_type,
            size_bytes: u64::try_from(bytes.len()).ok(),
            captured_bytes: u64::try_from(captured).unwrap_or(u64::MAX),
            encoding: BodyEncoding::Omitted,
            text: None,
            truncated,
            omitted_reason: Some("not_utf8".to_owned()),
        },
    }
}

fn empty_body() -> BodyCapture {
    BodyCapture {
        content_type: None,
        size_bytes: Some(0),
        captured_bytes: 0,
        encoding: BodyEncoding::Utf8,
        text: Some(String::new()),
        truncated: false,
        omitted_reason: None,
    }
}

pub(crate) fn offset_us(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn lock_map(
    map: &Mutex<HashMap<String, SharedCollector>>,
) -> std::sync::MutexGuard<'_, HashMap<String, SharedCollector>> {
    match map.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn lock_side(side: &Mutex<SideTable>) -> std::sync::MutexGuard<'_, SideTable> {
    match side.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn lock_collector_inner(collector: &Arc<Mutex<Collector>>) -> std::sync::MutexGuard<'_, Collector> {
    match collector.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
