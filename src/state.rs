use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorkerStatus {
    Waiting,
    Running,
    Done,
    #[allow(dead_code)]
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
    /// Byte offset of the section's first content byte within the file
    pub offset:  u64,
    /// 1-based line number of the section's first content line within the file
    pub line:    u64,
    pub value:   String,
}

pub struct AppState {
    pub net_bytes_done:   AtomicU64,
    pub net_bytes_total:  AtomicU64,
    pub transfer_label:   &'static str,
    pub workers:          Mutex<Vec<Arc<WorkerState>>>,
    pub results:          Mutex<Vec<ParseResult>>,
    pub remote:           bool,
    complete:             AtomicBool,
    cancelled:            AtomicBool,
    silent:               bool,
}

impl AppState {
    pub fn new(
        file_size:      u64,
        remote:         bool,
        transfer_label: &'static str,
        silent:         bool,
    ) -> Self {
        Self {
            net_bytes_done:  AtomicU64::new(0),
            net_bytes_total: AtomicU64::new(file_size),
            transfer_label,
            workers:         Mutex::new(Vec::new()),
            results:         Mutex::new(Vec::new()),
            remote,
            complete:        AtomicBool::new(false),
            cancelled:       AtomicBool::new(false),
            silent,
        }
    }

    /// Print to stderr only when not in GUI mode.
    pub fn log(&self, msg: &str) {
        if !self.silent {
            eprintln!("{msg}");
        }
    }

    /// Returns the transfer fraction in 0.0..=1.0, or None when the total
    /// is unknown (e.g. a URL with no Content-Length header).
    pub fn net_progress(&self) -> Option<f32> {
        let done  = self.net_bytes_done.load(Ordering::Relaxed) as f32;
        let total = self.net_bytes_total.load(Ordering::Relaxed) as f32;
        if total == 0.0 { None } else { Some((done / total).clamp(0.0, 1.0)) }
    }

    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Relaxed)
    }

    pub fn set_complete(&self) {
        self.complete.store(true, Ordering::Relaxed);
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
