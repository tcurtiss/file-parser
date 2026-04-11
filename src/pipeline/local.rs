use std::{path::Path, sync::Arc};

use anyhow::Result;
use rayon::prelude::*;

use crate::{
    boundaries::scan_boundaries,
    patterns::compile_all,
    state::AppState,
    worker::parse_section,
};

/// Local file pipeline: mmap the file, scan section boundaries, then parse
/// sections in parallel using rayon.
///
/// TODO: wire in scan_boundaries() and parse_section() once implemented.
pub fn run(path: &Path, state: Arc<AppState>, _workers: usize) -> Result<()> {
    use memmap2::MmapOptions;
    use std::fs::File;

    let file = File::open(path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };

    // Pass 1 — single-threaded linear scan for section boundaries
    let boundaries = scan_boundaries(&mmap);

    if boundaries.is_empty() {
        eprintln!("warning: no section boundaries found");
        state.set_complete();
        return Ok(());
    }

    // Compile patterns once; workers share the read-only databases
    let compiled = compile_all()?;

    // Pass 2 — parallel parse, one rayon task per section
    let all_results: Vec<_> = boundaries
        .par_iter()
        .map(|boundary| {
            let compiled_section = &compiled[boundary.section_idx];
            parse_section(&mmap, boundary, compiled_section, &state)
        })
        .collect();

    // Merge results into shared state
    let mut results = state.results.lock().unwrap();
    for batch in all_results {
        results.extend(batch);
    }
    results.sort_by_key(|r| r.offset);

    state.net_bytes_done
        .store(mmap.len() as u64, std::sync::atomic::Ordering::Relaxed);
    state.set_complete();
    Ok(())
}
