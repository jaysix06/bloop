//! Background decoding with a small LRU of decoded pictures, so stepping to
//! a neighbour shows it on the very next frame.
use crate::{
    decode::{self, Decoded},
    media::Entry,
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
};
use zgx::image::ImageData;

/// Decoded pictures kept in memory.
const BUDGET: usize = 768 * 1024 * 1024;

pub struct Loaded {
    pub path: PathBuf,
    pub result: Result<Arc<Decoded>, String>,
}

pub struct Thumb {
    pub path: PathBuf,
    pub image: Option<Arc<ImageData>>,
}

pub struct Loader {
    jobs: async_channel::Sender<Entry>,
    thumb_jobs: async_channel::Sender<PathBuf>,
    pub results: async_channel::Receiver<Loaded>,
    pub thumbs: async_channel::Receiver<Thumb>,
    /// Paths still worth decoding; workers skip anything else.
    wanted: Arc<Mutex<HashSet<PathBuf>>>,
    inflight: RefCell<HashSet<PathBuf>>,
    cache: RefCell<HashMap<PathBuf, (Arc<Decoded>, u64)>>,
    tick: std::cell::Cell<u64>,
}

impl Loader {
    pub fn new() -> Self {
        let (jobs, job_rx) = async_channel::unbounded::<Entry>();
        let (out, results) = async_channel::unbounded();
        let wanted = Arc::new(Mutex::new(HashSet::new()));
        let workers = thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 4);
        for i in 0..workers {
            let (job_rx, out, wanted) = (job_rx.clone(), out.clone(), wanted.clone());
            thread::Builder::new()
                .name(format!("bloop-decode-{i}"))
                .spawn(move || {
                    while let Ok(entry) = job_rx.recv_blocking() {
                        if !wanted.lock().unwrap().contains(&entry.path) {
                            let _ = out.send_blocking(Loaded {
                                path: entry.path,
                                result: Err("skipped".into()),
                            });
                            continue;
                        }
                        let result = decode::decode(&entry).map(Arc::new);
                        let _ = out.send_blocking(Loaded { path: entry.path, result });
                    }
                })
                .expect("spawn decoder");
        }
        let (thumb_jobs, thumb_rx) = async_channel::unbounded::<PathBuf>();
        let (thumb_out, thumbs) = async_channel::unbounded();
        for i in 0..2 {
            let (rx, out) = (thumb_rx.clone(), thumb_out.clone());
            thread::Builder::new()
                .name(format!("bloop-thumb-{i}"))
                .spawn(move || {
                    crate::wic::com_init_sta();
                    while let Ok(path) = rx.recv_blocking() {
                        let image = crate::shell::thumbnail(&path, 128);
                        let _ = out.send_blocking(Thumb { path, image });
                    }
                })
                .expect("spawn thumbnailer");
        }
        Self {
            jobs,
            thumb_jobs,
            results,
            thumbs,
            wanted,
            inflight: Default::default(),
            cache: Default::default(),
            tick: Default::default(),
        }
    }

    pub fn cached(&self, path: &PathBuf) -> Option<Arc<Decoded>> {
        let mut cache = self.cache.borrow_mut();
        let t = self.tick.get() + 1;
        self.tick.set(t);
        cache.get_mut(path).map(|(d, used)| {
            *used = t;
            d.clone()
        })
    }

    /// Ask for these, most important first. Anything not listed is
    /// dropped from the queue (not from the cache).
    pub fn want(&self, entries: &[Entry]) {
        {
            let mut wanted = self.wanted.lock().unwrap();
            wanted.clear();
            wanted.extend(entries.iter().map(|e| e.path.clone()));
        }
        for entry in entries {
            if entry.kind == crate::media::MediaKind::Video {
                continue;
            }
            if self.cache.borrow().contains_key(&entry.path) {
                continue;
            }
            if self.inflight.borrow_mut().insert(entry.path.clone()) {
                let _ = self.jobs.try_send(entry.clone());
            }
        }
    }

    /// A result arrived: cache it and say whether it's usable.
    pub fn accept(&self, loaded: &Loaded) {
        self.inflight.borrow_mut().remove(&loaded.path);
        if let Ok(decoded) = &loaded.result {
            let t = self.tick.get() + 1;
            self.tick.set(t);
            let mut cache = self.cache.borrow_mut();
            cache.insert(loaded.path.clone(), (decoded.clone(), t));
            let wanted = self.wanted.lock().unwrap().clone();
            let size = |d: &Decoded| d.width as usize * d.height as usize * 4 * d.frames.min(64);
            let mut total: usize = cache.values().map(|(d, _)| size(d)).sum();
            while total > BUDGET {
                let victim = cache
                    .iter()
                    .filter(|(p, _)| !wanted.contains(*p))
                    .min_by_key(|(_, (_, used))| *used)
                    .map(|(p, _)| p.clone());
                let Some(victim) = victim else { break };
                if let Some((d, _)) = cache.remove(&victim) {
                    total -= size(&d);
                }
            }
        }
    }

    pub fn forget(&self, path: &PathBuf) {
        self.cache.borrow_mut().remove(path);
    }

    pub fn thumbnail(&self, path: PathBuf) {
        let _ = self.thumb_jobs.try_send(path);
    }
}
