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
use memmap2::MmapOptions;

use crate::{
    boundaries::{SectionBoundary, compile_header_patterns, find_header_hits},
    patterns::compile_all,
    sections::SECTIONS,
    source::Source,
    state::AppState,
    worker::parse_section,
};

const READ_CHUNK: usize = 16 * 1024 * 1024; // 16 MB — keeps the network pipe full

/// Remote pipeline: handles both network-mounted files and URLs.
///   1. Open the source (file or HTTP) and stream to a local temp file
///   2. Scan section boundaries behind the writer (pass 1) — incremental,
///      only newly written bytes are re-scanned on each tick
///   3. Dispatch each completed section to the worker as soon as its end
///      boundary is known (i.e. the next header has been seen, or writing is done)
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

    // Compile patterns once; workers share the read-only regexes.
    let header_patterns = compile_header_patterns();
    let compiled        = compile_all()?;

    // ── Incremental scan state ──────────────────────────────────────────────
    //
    // scan_cursor  — file offset up to which we have already scanned for
    //                headers.  Always aligned to a newline boundary so that no
    //                header line is split across two scan windows.
    //
    // all_hits     — every header hit found so far, sorted by file offset.
    //                Each entry: (header_start, content_start, section_idx).
    //
    // dispatched   — number of entries from the front of all_hits that have
    //                already been handed to parse_section().
    //
    // newlines_*   — running newline counter used to fill SectionBoundary.line_start.
    let mut scan_cursor:   usize = 0;
    let mut all_hits:      Vec<(usize, usize, usize)> = Vec::new();
    let mut dispatched:    usize = 0;
    let mut newlines_seen: u64   = 0;
    let mut newlines_pos:  usize = 0;

    let mut local_results = Vec::new();

    // ── Scan-behind loop ────────────────────────────────────────────────────
    loop {
        let written = bytes_written.load(Ordering::Acquire) as usize;
        let done    = write_done.load(Ordering::Acquire);

        if written > scan_cursor && !state.is_cancelled() {
            // Map the temp file up to `written` bytes so we can read it.
            let tmp_file = std::fs::File::open(&tmp_path)?;
            let mmap = unsafe { MmapOptions::new().len(written).map(&tmp_file)? };

            // Only scan up to the last newline in the new data to guarantee
            // no header line is split at the edge of our scan window.
            // When writing is complete we scan all remaining bytes.
            let scan_end = if done {
                written
            } else {
                mmap[scan_cursor..written]
                    .iter()
                    .rposition(|&b| b == b'\n')
                    .map(|rel| scan_cursor + rel + 1)
                    .unwrap_or(scan_cursor) // no newline yet — wait for more data
            };

            if scan_end > scan_cursor {
                let new_hits = find_header_hits(
                    &mmap[scan_cursor..scan_end],
                    scan_cursor,
                    &header_patterns,
                );
                all_hits.extend(new_hits);
                all_hits.sort_by_key(|&(start, _, _)| start);
                scan_cursor = scan_end;
            }

            // A section is complete when its end boundary is known:
            //   • any section that is not the last one in all_hits (its end =
            //     the next header's start), or
            //   • the last section once writing is done.
            let complete_count = if done {
                all_hits.len()
            } else {
                all_hits.len().saturating_sub(1)
            };

            while dispatched < complete_count {
                let (_, content_start, section_idx) = all_hits[dispatched];
                let content_end = if dispatched + 1 < all_hits.len() {
                    all_hits[dispatched + 1].0
                } else {
                    written
                };

                // Count newlines from where we last stopped up to this section's
                // content start.  This keeps the total newline-counting work O(n).
                newlines_seen += mmap[newlines_pos..content_start]
                    .iter()
                    .filter(|&&b| b == b'\n')
                    .count() as u64;
                newlines_pos = content_start;

                let boundary = SectionBoundary {
                    section_idx,
                    name:       SECTIONS[section_idx].name.to_string(),
                    start:      content_start as u64,
                    end:        content_end   as u64,
                    line_start: newlines_seen + 1,
                };

                let results = parse_section(&mmap, &boundary, &compiled[section_idx], &state);
                local_results.extend(results);
                dispatched += 1;
            }
        }

        if done || state.is_cancelled() {
            break;
        }

        thread::sleep(Duration::from_millis(100));
    }

    // Merge into shared state, sorted by file offset (matches local pipeline).
    {
        let mut results = state.results.lock().unwrap();
        results.extend(local_results);
        results.sort_by_key(|r| r.offset);
    }

    std::fs::remove_file(&tmp_path).ok();
    state.set_complete();
    Ok(())
}
