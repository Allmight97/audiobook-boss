//! One loader per cover. Every view that shows a cover, Lookup Apply, and a
//! typed cover URL read it here: each cover is fetched or read once per size,
//! sized from the downloaded bytes, and kept in a bounded cache. Concurrent
//! requests for one cover share its load.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use tokio::sync::{OnceCell, Semaphore};

use crate::errors::{AppError, Result};

/// How many covers are kept: a whole Audible library's thumbnails. Small
/// covers are a few kilobytes, full ones about a hundred, so this stays
/// under about fifty megabytes.
const CACHED_COVERS: usize = 512;
const REMOTE_FETCHES: usize = 12;
const LOCAL_READS: usize = 2;

type Fetch = dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send>> + Send + Sync;

/// A cover's bytes, shared between the cache and its readers.
pub(crate) type Cover = Arc<[u8]>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Key {
    /// Write-ready bytes of a remote cover.
    RemoteFull(String),
    RemoteSmall(String),
    /// An audio file's embedded cover, as of the file's last change.
    EmbeddedSmall {
        path: PathBuf,
        modified: Option<SystemTime>,
        len: u64,
    },
}

type Slot = Arc<OnceCell<Option<Cover>>>;

#[derive(Default)]
struct Entries {
    slots: HashMap<Key, Slot>,
    order: VecDeque<Key>,
}

impl Entries {
    fn slot(&mut self, key: Key) -> Slot {
        if let Some(slot) = self.slots.get(&key) {
            let slot = Arc::clone(slot);
            self.order.retain(|kept| kept != &key);
            self.order.push_back(key);
            return slot;
        }
        while self.order.len() >= CACHED_COVERS {
            if let Some(oldest) = self.order.pop_front() {
                self.slots.remove(&oldest);
            }
        }
        let slot = Slot::default();
        self.slots.insert(key.clone(), Arc::clone(&slot));
        self.order.push_back(key);
        slot
    }
}

#[derive(Clone)]
pub(crate) struct CoverService {
    inner: Arc<Inner>,
}

struct Inner {
    entries: Mutex<Entries>,
    remote: Semaphore,
    local: Semaphore,
    fetch: Box<Fetch>,
}

impl CoverService {
    /// `fetch` returns a remote cover's downloaded bytes.
    pub(crate) fn new(fetch: Box<Fetch>) -> Self {
        Self {
            inner: Arc::new(Inner {
                entries: Mutex::default(),
                remote: Semaphore::new(REMOTE_FETCHES),
                local: Semaphore::new(LOCAL_READS),
                fetch,
            }),
        }
    }

    pub(crate) fn live() -> Self {
        Self::new(Box::new(|url| {
            Box::pin(crate::cover_source::download_cover_art(url))
        }))
    }

    /// A remote cover's write-ready bytes.
    pub(crate) async fn remote_full(&self, url: &str) -> Result<Cover> {
        let fetched = self
            .load(Key::RemoteFull(url.to_string()), async {
                let downloaded = self.download(url).await?;
                blocking(move || crate::metadata::optimize_cover_art(&downloaded).map(Some)).await
            })
            .await?;
        fetched.ok_or_else(|| AppError::General("The cover could not be loaded.".into()))
    }

    /// A remote cover sized for a thumbnail, made from its downloaded bytes.
    pub(crate) async fn remote_small(&self, url: &str) -> Result<Cover> {
        let small = self
            .load(Key::RemoteSmall(url.to_string()), async {
                let downloaded = self.download(url).await?;
                blocking(move || crate::metadata::render_display_thumbnail(&downloaded).map(Some))
                    .await
            })
            .await?;
        small.ok_or_else(|| AppError::General("The cover could not be loaded.".into()))
    }

    async fn download(&self, url: &str) -> Result<Vec<u8>> {
        let _turn = self.inner.remote.acquire().await;
        (self.inner.fetch)(url.to_string()).await
    }

    /// An audio file's embedded cover sized for a thumbnail; `None` when it
    /// has none. `path` is already validated.
    pub(crate) async fn embedded_small(&self, path: &Path) -> Result<Option<Cover>> {
        let stamp = std::fs::metadata(path).map_err(AppError::Io)?;
        let key = Key::EmbeddedSmall {
            path: path.to_path_buf(),
            modified: stamp.modified().ok(),
            len: stamp.len(),
        };
        self.load(key, async {
            let _turn = self.inner.local.acquire().await;
            let path = path.to_path_buf();
            blocking(move || crate::metadata::read_audio_cover_thumbnail(&path)).await
        })
        .await
    }

    /// Joins the load already under way for `key`, or starts `load`. A
    /// failed load is not kept, so the next request tries again.
    async fn load(
        &self,
        key: Key,
        load: impl Future<Output = Result<Option<Vec<u8>>>>,
    ) -> Result<Option<Cover>> {
        let slot = self
            .inner
            .entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .slot(key);
        slot.get_or_try_init(|| async { load.await.map(|bytes| bytes.map(Cover::from)) })
            .await
            .cloned()
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    #[expect(clippy::disallowed_methods, reason = "joined by blocking")]
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| AppError::General(format!("Cover task failed: {error}")))?
}

#[cfg(test)]
#[path = "cover_service_tests.rs"]
mod cover_service_tests;
