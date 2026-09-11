//! A memory-bounded `ImageCache`.
//!
//! GPUI ships only `RetainAllImageCache`, which never evicts. Eagle thumbnails
//! decode to roughly 600 KB of RGBA each, so retaining a 7,000-asset library
//! would climb past 4 GB. This is that cache with an LRU bound bolted on.
//!
//! The *loading* half is deliberately identical to GPUI's: the decode already
//! happens on the background executor, and it already sniffs image format by
//! content (which is what makes Eagle's WebP-masquerading-as-PNG thumbnails
//! work). Only the eviction policy is ours.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use futures::FutureExt as _;
use gpui::{
    hash, prelude::*, App, Asset as _, AssetLogger, Entity, ImageAssetLoader, ImageCache,
    ImageCacheError, ImageCacheItem, RenderImage, Resource, Window,
};

/// Default ceiling on decoded thumbnail bytes held in memory.
pub const DEFAULT_BUDGET_BYTES: usize = 512 * 1024 * 1024;

struct Entry {
    item: ImageCacheItem,
    /// Decoded size, counted once the image finishes loading.
    bytes: usize,
    /// Access clock, mirrored as a key in `lru`.
    stamp: u64,
}

pub struct LruImageCache {
    entries: HashMap<u64, Entry>,
    /// `stamp -> key`, so the least-recently-used entry is `lru.first_key_value()`.
    lru: BTreeMap<u64, u64>,
    clock: u64,
    bytes: usize,
    budget: usize,
}

impl LruImageCache {
    pub fn new(budget: usize, cx: &mut App) -> Entity<Self> {
        let entity = cx.new(|_| Self {
            entries: HashMap::new(),
            lru: BTreeMap::new(),
            clock: 0,
            bytes: 0,
            budget: budget.max(64 * 1024 * 1024),
        });

        // Release every atlas slot when the cache itself goes away.
        cx.observe_release(&entity, |cache, cx| {
            for (_, mut entry) in std::mem::take(&mut cache.entries) {
                if let Some(Ok(image)) = entry.item.get() {
                    cx.drop_image(image, None);
                }
            }
            cache.lru.clear();
            cache.bytes = 0;
        })
        .detach();

        entity
    }

    fn touch(&mut self, key: u64) {
        self.clock += 1;
        let stamp = self.clock;
        if let Some(entry) = self.entries.get_mut(&key) {
            self.lru.remove(&entry.stamp);
            entry.stamp = stamp;
            self.lru.insert(stamp, key);
        }
    }

    /// Evict least-recently-used loaded entries until back under budget.
    ///
    /// `protect` is the key just handed to the renderer this frame; evicting it
    /// would guarantee an immediate reload.
    fn evict(&mut self, protect: u64, window: &mut Window, cx: &mut App) {
        while self.bytes > self.budget {
            let Some((&stamp, &key)) = self.lru.iter().next() else {
                break;
            };
            if key == protect {
                // Nothing else left to give up.
                if self.lru.len() == 1 {
                    break;
                }
                // Skip it by pushing it to the front of the queue.
                self.touch(key);
                continue;
            }

            self.lru.remove(&stamp);
            let Some(mut entry) = self.entries.remove(&key) else {
                continue;
            };
            self.bytes = self.bytes.saturating_sub(entry.bytes);
            if let Some(Ok(image)) = entry.item.get() {
                cx.drop_image(image, Some(window));
            }
        }
    }
}

/// Decoded byte cost of an image across all its frames.
fn decoded_bytes(image: &Arc<RenderImage>) -> usize {
    (0..image.frame_count())
        .filter_map(|i| image.as_bytes(i))
        .map(|b| b.len())
        .sum()
}

impl ImageCache for LruImageCache {
    fn load(
        &mut self,
        resource: &Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<Arc<RenderImage>, ImageCacheError>> {
        let key = hash(resource);

        if self.entries.contains_key(&key) {
            self.touch(key);
            let entry = self.entries.get_mut(&key)?;
            let was_accounted = entry.bytes > 0;
            let result = entry.item.get();

            // A `Loading` entry becomes `Loaded` inside `get()`. That is the
            // first moment its decoded size is knowable, so bill it here.
            if let Some(Ok(image)) = &result {
                if !was_accounted {
                    let bytes = decoded_bytes(image);
                    entry.bytes = bytes;
                    self.bytes += bytes;
                    self.evict(key, window, cx);
                }
            }
            return result;
        }

        // Same loading path as `RetainAllImageCache`: decode off the main
        // thread, then nudge the view that asked for it once it lands.
        let fut = AssetLogger::<ImageAssetLoader>::load(resource.clone(), cx);
        let task = cx.background_executor().spawn(fut).shared();

        self.clock += 1;
        let stamp = self.clock;
        self.entries.insert(
            key,
            Entry {
                item: ImageCacheItem::Loading(task.clone()),
                bytes: 0,
                stamp,
            },
        );
        self.lru.insert(stamp, key);

        let entity = window.current_view();
        window
            .spawn(cx, async move |cx| {
                _ = task.await;
                cx.on_next_frame(move |_, cx| cx.notify(entity));
            })
            .detach();

        None
    }
}
