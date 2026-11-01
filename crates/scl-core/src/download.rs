//! Parallel downloader with SHA-1 verification and progress reporting.
//!
//! Every long-running operation in the launcher streams [`TaskEvent`]s over a
//! `std::sync::mpsc` channel. The UI thread forwards those into the GPUI view,
//! which keeps the engine completely free of UI dependencies.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

impl DownloadItem {
    pub fn new(url: impl Into<String>, dest: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            dest: dest.into(),
            sha1: None,
            size: None,
        }
    }

    pub fn with_sha1(mut self, sha1: Option<String>) -> Self {
        self.sha1 = sha1;
        self
    }

    pub fn with_size(mut self, size: Option<u64>) -> Self {
        self.size = size;
        self
    }
}

#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// A new phase started, e.g. "正在校验游戏文件".
    Stage(String),
    /// Aggregate progress of the current phase.
    Progress {
        done: usize,
        total: usize,
        bytes: u64,
        current: String,
    },
    /// Human readable line for the log panel.
    Log(String),
    Done {
        downloaded: usize,
        skipped: usize,
        failed: usize,
    },
    Failed(String),
}

#[derive(Debug, Clone, Default)]
pub struct DownloadReport {
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub bytes: u64,
    pub errors: Vec<String>,
}

struct Shared {
    tx: Sender<TaskEvent>,
    done: AtomicUsize,
    failed: AtomicUsize,
    bytes: AtomicU64,
    total: usize,
    started: Instant,
    last_emit_ms: AtomicU64,
    errors: Mutex<Vec<String>>,
    cancel: Arc<AtomicBool>,
}

impl Shared {
    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    fn emit(&self, current: &str) {
        let _ = self.tx.send(TaskEvent::Progress {
            done: self.done.load(Ordering::Relaxed),
            total: self.total,
            bytes: self.bytes.load(Ordering::Relaxed),
            current: current.to_string(),
        });
    }

    fn emit_throttled(&self, current: &str) {
        let now = self.elapsed_ms();
        let last = self.last_emit_ms.load(Ordering::Relaxed);
        if now.saturating_sub(last) >= 120 {
            self.last_emit_ms.store(now, Ordering::Relaxed);
            self.emit(current);
        }
    }
}

/// Returns true when the file already exists and matches the expected hash.
pub fn is_complete(item: &DownloadItem) -> bool {
    let Ok(meta) = std::fs::metadata(&item.dest) else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }
    if let Some(size) = item.size {
        if meta.len() != size {
            return false;
        }
    }
    match &item.sha1 {
        Some(expected) if !expected.is_empty() => {
            // Size match is a cheap pre-check; only hash when size is unknown.
            if item.size.is_none() {
                matches!(crate::net::sha1_of_file(&item.dest), Ok(actual) if actual == *expected)
            } else {
                true
            }
        }
        _ => true,
    }
}

fn file_name_of(item: &DownloadItem) -> String {
    item.dest
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| item.url.clone())
}

/// Downloads `items` using `threads` workers. Blocking; call from a worker thread.
pub fn download_all(
    items: Vec<DownloadItem>,
    threads: usize,
    tx: Sender<TaskEvent>,
    cancel: Arc<AtomicBool>,
) -> DownloadReport {
    let mut report = DownloadReport::default();

    let mut pending: Vec<DownloadItem> = Vec::with_capacity(items.len());
    for item in items {
        if is_complete(&item) {
            report.skipped += 1;
        } else {
            pending.push(item);
        }
    }
    let total = pending.len();
    if total == 0 {
        let _ = tx.send(TaskEvent::Progress {
            done: 0,
            total: 0,
            bytes: 0,
            current: String::new(),
        });
        let _ = tx.send(TaskEvent::Done {
            downloaded: 0,
            skipped: report.skipped,
            failed: 0,
        });
        return report;
    }

    let shared = Arc::new(Shared {
        tx,
        done: AtomicUsize::new(0),
        failed: AtomicUsize::new(0),
        bytes: AtomicU64::new(0),
        total,
        started: Instant::now(),
        last_emit_ms: AtomicU64::new(0),
        errors: Mutex::new(Vec::new()),
        cancel,
    });
    let queue = Arc::new(Mutex::new(pending));
    let next = Arc::new(AtomicUsize::new(0));
    let workers = threads.clamp(1, 16).min(total);

    shared.emit("");

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let shared = Arc::clone(&shared);
            let queue = Arc::clone(&queue);
            let next = Arc::clone(&next);
            scope.spawn(move || loop {
                if shared.cancel.load(Ordering::Relaxed) {
                    break;
                }
                let idx = next.fetch_add(1, Ordering::Relaxed);
                let item = {
                    let guard = queue.lock().unwrap();
                    match guard.get(idx) {
                        Some(item) => item.clone(),
                        None => break,
                    }
                };
                let name = file_name_of(&item);
                let shared_ref = &shared;
                let result = crate::net::download_to(&item.url, &item.dest, |n| {
                    shared_ref.bytes.fetch_add(n, Ordering::Relaxed);
                    shared_ref.emit_throttled(&name);
                });

                match result {
                    Ok(_) => {
                        if let Some(expected) = &item.sha1 {
                            if !expected.is_empty() {
                                match crate::net::sha1_of_file(&item.dest) {
                                    Ok(actual) if actual == *expected => {}
                                    Ok(_) => {
                                        let _ = std::fs::remove_file(&item.dest);
                                        shared.failed.fetch_add(1, Ordering::Relaxed);
                                        shared
                                            .errors
                                            .lock()
                                            .unwrap()
                                            .push(format!("{name}: SHA-1 校验失败"));
                                        shared.done.fetch_add(1, Ordering::Relaxed);
                                        shared.emit(&name);
                                        continue;
                                    }
                                    Err(e) => {
                                        shared
                                            .errors
                                            .lock()
                                            .unwrap()
                                            .push(format!("{name}: {e}"));
                                    }
                                }
                            }
                        }
                        shared.done.fetch_add(1, Ordering::Relaxed);
                        shared.emit(&name);
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(item.dest.with_extension("part"));
                        shared.failed.fetch_add(1, Ordering::Relaxed);
                        shared
                            .errors
                            .lock()
                            .unwrap()
                            .push(format!("{name}: {e}"));
                        shared.done.fetch_add(1, Ordering::Relaxed);
                        shared.emit(&name);
                    }
                }
            });
        }
    });

    report.downloaded = shared
        .done
        .load(Ordering::Relaxed)
        .saturating_sub(shared.failed.load(Ordering::Relaxed));
    report.failed = shared.failed.load(Ordering::Relaxed);
    report.bytes = shared.bytes.load(Ordering::Relaxed);
    report.errors = shared.errors.lock().unwrap().clone();

    let _ = shared.tx.send(TaskEvent::Done {
        downloaded: report.downloaded,
        skipped: report.skipped,
        failed: report.failed,
    });

    report
}

/// Small helper for writing a single file (version JSON, log config, ...).
pub fn write_file(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}
