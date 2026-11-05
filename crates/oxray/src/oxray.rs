//! Handle, builder, and mode selection.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Once};

use axum::Router;

use crate::Capture;
use crate::api;
use crate::layer::OxrayLayer;
use crate::redact::Redactor;
use crate::store::MemoryStore;

const DEFAULT_BODY_LIMIT: usize = 64 * 1024;
const DEFAULT_MAX_CAPTURES: usize = 1_000;
const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

/// Whether Oxray is recording requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// Pass-through. The mounted router answers 404.
    Disabled,
    /// Record requests and guard the mounted router.
    Dev,
}

/// Startup configuration error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// `body_limit` was zero.
    #[error("body_limit must be greater than zero")]
    BodyLimitZero,
    /// `max_captures` was zero.
    #[error("max_captures must be greater than zero")]
    MaxCapturesZero,
    /// `max_bytes` was zero.
    #[error("max_bytes must be greater than zero")]
    MaxBytesZero,
    /// `mount` did not start with `/`, was `/`, or ended with `/`.
    #[error("mount path must start with / and must not be / or end with /")]
    MountPath,
}

/// Cloneable handle shared by the layer and the router.
#[derive(Clone)]
pub struct Oxray {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) mode: Mode,
    pub(crate) mount: String,
    pub(crate) body_limit: usize,
    pub(crate) response_header: bool,
    pub(crate) redactor: Redactor,
    pub(crate) store: MemoryStore,
    pub(crate) fail_store: AtomicBool,
    pub(crate) fail_redact: AtomicBool,
    pub(crate) warned_store: AtomicBool,
    pub(crate) warned_redact: AtomicBool,
    pub(crate) store_warns: AtomicU32,
    pub(crate) redact_warns: AtomicU32,
}

/// Builder for [`Oxray`].
#[derive(Debug)]
#[non_exhaustive]
pub struct OxrayBuilder {
    body_limit: usize,
    max_captures: usize,
    max_bytes: usize,
    mount: String,
    response_header: bool,
    redact_headers: Vec<String>,
    redact_keys: Vec<String>,
}

impl Oxray {
    /// Starts a builder with the default limits.
    pub fn builder() -> OxrayBuilder {
        OxrayBuilder {
            body_limit: DEFAULT_BODY_LIMIT,
            max_captures: DEFAULT_MAX_CAPTURES,
            max_bytes: DEFAULT_MAX_BYTES,
            mount: "/_oxray".to_owned(),
            response_header: true,
            redact_headers: Vec::new(),
            redact_keys: Vec::new(),
        }
    }

    /// Resolved mode.
    pub fn mode(&self) -> Mode {
        self.inner.mode
    }

    /// Tower layer that records requests outside the mount path.
    pub fn layer(&self) -> OxrayLayer {
        OxrayLayer {
            oxray: self.clone(),
        }
    }

    /// Router mounted at the configured path.
    pub fn router(&self) -> Router {
        api::router(self.clone())
    }

    /// Redacts `capture` with this handle's rules.
    pub fn redact(&self, capture: &mut Capture) {
        self.inner.redactor.apply(capture);
    }

    pub(crate) fn store_capture(&self, mut capture: Capture) {
        if self.inner.fail_redact.load(Ordering::Relaxed) {
            self.warn_once(
                &self.inner.warned_redact,
                &self.inner.redact_warns,
                "redactor",
            );
            return;
        }
        self.inner.redactor.apply(&mut capture);
        if self.inner.fail_store.load(Ordering::Relaxed) {
            self.warn_once(&self.inner.warned_store, &self.inner.store_warns, "store");
            return;
        }
        self.inner.store.insert(capture);
    }

    fn warn_once(&self, flag: &AtomicBool, count: &AtomicU32, kind: &str) {
        if !flag.swap(true, Ordering::Relaxed) {
            count.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(target: "oxray", kind, "discarding a capture after an internal failure");
        }
    }

    /// Arms the next store failure. Only compiled for the hidden fault feature.
    #[cfg(feature = "__fault-injection")]
    pub fn fault_fail_store(&self) {
        self.inner.fail_store.store(true, Ordering::Relaxed);
    }

    /// Arms the next redactor failure. Only compiled for the hidden fault feature.
    #[cfg(feature = "__fault-injection")]
    pub fn fault_fail_redact(&self) {
        self.inner.fail_redact.store(true, Ordering::Relaxed);
    }

    /// How many store warnings this handle has emitted.
    #[cfg(feature = "__fault-injection")]
    pub fn fault_store_warns(&self) -> u32 {
        self.inner.store_warns.load(Ordering::Relaxed)
    }

    /// How many redactor warnings this handle has emitted.
    #[cfg(feature = "__fault-injection")]
    pub fn fault_redact_warns(&self) -> u32 {
        self.inner.redact_warns.load(Ordering::Relaxed)
    }
}

impl OxrayBuilder {
    /// Maximum body bytes kept on a capture.
    pub fn body_limit(mut self, bytes: usize) -> Self {
        self.body_limit = bytes;
        self
    }

    /// Maximum stored captures.
    pub fn max_captures(mut self, count: usize) -> Self {
        self.max_captures = count;
        self
    }

    /// Maximum encoded bytes stored.
    pub fn max_bytes(mut self, bytes: usize) -> Self {
        self.max_bytes = bytes;
        self
    }

    /// Mount path for the API. Defaults to `/_oxray`.
    pub fn mount(mut self, path: impl Into<String>) -> Self {
        self.mount = path.into();
        self
    }

    /// Adds `X-Oxray-Id` when `enabled` is true.
    pub fn response_header(mut self, enabled: bool) -> Self {
        self.response_header = enabled;
        self
    }

    /// Extra header names to redact.
    pub fn redact_headers<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.redact_headers
            .extend(names.into_iter().map(Into::into));
        self
    }

    /// Extra object keys to redact.
    pub fn redact_keys<I, S>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.redact_keys.extend(keys.into_iter().map(Into::into));
        self
    }

    /// Builds the handle, or rejects an empty limit.
    pub fn build(self) -> Result<Oxray, Error> {
        if self.body_limit == 0 {
            return Err(Error::BodyLimitZero);
        }
        if self.max_captures == 0 {
            return Err(Error::MaxCapturesZero);
        }
        if self.max_bytes == 0 {
            return Err(Error::MaxBytesZero);
        }
        if !valid_mount(&self.mount) {
            return Err(Error::MountPath);
        }
        install_panic_hook();
        let mode = resolve_mode(std::env::var("OXRAY").ok().as_deref());
        Ok(Oxray {
            inner: Arc::new(Inner {
                mode,
                mount: self.mount,
                body_limit: self.body_limit,
                response_header: self.response_header,
                redactor: Redactor::new(&self.redact_headers, &self.redact_keys),
                store: MemoryStore::new(self.max_captures, self.max_bytes),
                fail_store: AtomicBool::new(false),
                fail_redact: AtomicBool::new(false),
                warned_store: AtomicBool::new(false),
                warned_redact: AtomicBool::new(false),
                store_warns: AtomicU32::new(0),
                redact_warns: AtomicU32::new(0),
            }),
        })
    }
}

fn valid_mount(mount: &str) -> bool {
    mount.starts_with('/') && mount.len() > 1 && !mount.ends_with('/')
}

pub(crate) fn resolve_mode(oxray_env: Option<&str>) -> Mode {
    if oxray_env == Some("off") || cfg!(oxray_release_profile) {
        Mode::Disabled
    } else {
        Mode::Dev
    }
}

fn install_panic_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
        }));
    });
}
