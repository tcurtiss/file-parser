use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorkerStatus {
    Waiting,
    Running,
    Done,
    Failed,
}

pub struct WorkerState {
    pub section_name: String,
    pub bytes_done:   AtomicU64,
    pub bytes_total:  AtomicU64,
    pub matches:      AtomicU64,
    pub status:       Mutex<WorkerStatus>,
}

impl WorkerState {
    pub fn new(section_name: String, bytes_total: u64) -> Self {
        Self {
            section_name,
            bytes_done:  AtomicU64::new(0),
            bytes_total: AtomicU64::new(bytes_total),
            matches:     AtomicU64::new(0),
            status:      Mutex::new(WorkerStatus::Waiting),
        }
    }

    pub fn progress(&self) -> f32 {
        let done  = self.bytes_done.load(Ordering::Relaxed) as f32;
        let total = self.bytes_total.load(Ordering::Relaxed) as f32;
        if total == 0.0 { 0.0 } else { (done / total).clamp(0.0, 1.0) }
    }
}

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub section: String,
    pub label:   String,
    pub offset:  u64,
    pub value:   String,
}

pub struct AppState {
    pub net_bytes_done:  AtomicU64,
    pub net_bytes_total: AtomicU64,
    pub workers:         Mutex<Vec<Arc<WorkerState>>>,
    pub results:         Mutex<Vec<ParseResult>>,
    pub remote:          bool,
    complete:            AtomicBool,
    silent:              bool,
}

impl AppState {
    pub fn new(file_size: u64, remote: bool, silent: bool) -> Self {
        Self {
            net_bytes_done:  AtomicU64::new(0),
            net_bytes_total: AtomicU64::new(file_size),
            workers:         Mutex::new(Vec::new()),
            results:         Mutex::new(Vec::new()),
            remote,
            complete:        AtomicBool::new(false),
            silent,
        }
    }

    /// Print to stderr only when not in GUI mode.
    pub fn log(&self, msg: &str) {
        if !self.silent {
            eprintln!("{msg}");
        }
    }

    pub fn net_progress(&self) -> f32 {
        let done  = self.net_bytes_done.load(Ordering::Relaxed) as f32;
        let total = self.net_bytes_total.load(Ordering::Relaxed) as f32;
        if total == 0.0 { 1.0 } else { (done / total).clamp(0.0, 1.0) }
    }

    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Relaxed)
    }

    pub fn set_complete(&self) {
        self.complete.store(true, Ordering::Relaxed);
    }
}
