use std::{
    io::Write,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use anyhow::Result;

use crate::{source::Source, state::AppState};

const READ_CHUNK: usize = 16 * 1024 * 1024; // 16 MB — keeps the network pipe full

/// Remote pipeline: handles both network-mounted files and URLs.
///   1. Open the source (file or HTTP) and stream to a local temp file
///   2. Scan section boundaries behind the writer (pass 1)
///   3. Dispatch completed sections to the worker pool
///
/// TODO: wire in scan_boundaries() and worker dispatch once implemented.
pub fn run(source: Source, state: Arc<AppState>, _workers: usize) -> Result<()> {
    use std::io::Read;

    let tmp_path = std::env::temp_dir().join("file-parser-staging");
    let mut dst  = std::fs::File::create(&tmp_path)?;

    // Open the source — for URLs this makes the HTTP request and reads
    // Content-Length from the response headers.
    let (mut src, content_length) = source.open_reader()?;

    // Update total bytes now that we may know it (URL Content-Length).
    if let Some(len) = content_length {
        state.net_bytes_total.store(len, Ordering::Relaxed);
    }

    // TODO: for Source::File on Linux, posix_fadvise(SEQUENTIAL) can be applied
    // here once we have a concrete File reference. Not applicable to HTTP readers.

    let bytes_written = Arc::new(AtomicU64::new(0));
    let write_done    = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // ── Copy thread: source → local temp ───────────────────────────────────
    {
        let bytes_written = Arc::clone(&bytes_written);
        let write_done    = Arc::clone(&write_done);
        let state         = Arc::clone(&state);

        thread::spawn(move || {
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                match src.read(&mut buf) {
                    Ok(0)  => break,
                    Ok(n)  => {
                        dst.write_all(&buf[..n]).expect("temp write failed");
                        dst.flush().expect("temp flush failed");
                        let total = bytes_written.fetch_add(n as u64, Ordering::Release) + n as u64;
                        state.net_bytes_done.store(total, Ordering::Relaxed);
                    }
                    Err(e) => { state.log(&format!("read error: {e}")); break; }
                }
            }
            write_done.store(true, Ordering::Release);
        });
    }

    // ── Scan thread: follows behind the copy thread ─────────────────────────
    // TODO: replace this polling loop with real boundary scanning and worker
    //       dispatch once scan_boundaries() and parse_section() are implemented.
    loop {
        let written = bytes_written.load(Ordering::Acquire);
        let done    = write_done.load(Ordering::Acquire);

        // TODO:
        // 1. open/mmap tmp_path up to `written` bytes
        // 2. run scan_boundaries() on the newly available range
        // 3. for each completed section, submit to worker pool
        // 4. collect results into state

        if done || state.is_cancelled() { break; }
        thread::sleep(Duration::from_millis(100));
        let _ = written;
    }

    std::fs::remove_file(&tmp_path).ok();
    state.set_complete();
    Ok(())
}
