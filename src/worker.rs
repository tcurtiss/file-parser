use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::{
    boundaries::SectionBoundary,
    patterns::CompiledSection,
    sections::SECTIONS,
    state::{AppState, ParseResult, WorkerState, WorkerStatus},
};

/// Parse a single section of the file, applying all compiled patterns for that section type.
/// Registers a `WorkerState` in `state`, updates progress as it runs, and returns all matches.
///
/// For each content pattern the matched `\d+` numbers are accumulated and a single
/// summary `ParseResult` is returned with the total as its value.
///
/// Designed to be called from a rayon thread pool — takes shared references only.
pub fn parse_section(
    data:     &[u8],
    boundary: &SectionBoundary,
    compiled: &CompiledSection,
    state:    &Arc<AppState>,
) -> Vec<ParseResult> {
    let section_data = &data[boundary.start as usize..boundary.end as usize];
    let section_def  = &SECTIONS[boundary.section_idx];

    // Register worker progress entry
    let worker = Arc::new(WorkerState::new(
        boundary.name.clone(),
        section_data.len() as u64,
    ));
    *worker.status.lock().unwrap() = WorkerStatus::Running;
    state.workers.lock().unwrap().push(Arc::clone(&worker));

    let mut results = Vec::new();

    for (label, re) in &compiled.patterns {
        let mut sum: u64 = 0;
        let mut count: u64 = 0;

        for caps in re.captures_iter(section_data) {
            // Group 1 holds the captured \d+
            if let Some(m) = caps.get(1) {
                if let Ok(s) = std::str::from_utf8(m.as_bytes()) {
                    if let Ok(n) = s.parse::<u64>() {
                        sum   += n;
                        count += 1;
                    }
                }
            }
        }

        worker.matches.fetch_add(count, Ordering::Relaxed);

        results.push(ParseResult {
            section: section_def.name.to_string(),
            label:   label.clone(),
            offset:  boundary.start,
            value:   sum.to_string(),
        });
    }

    worker.bytes_done.store(section_data.len() as u64, Ordering::Relaxed);
    *worker.status.lock().unwrap() = WorkerStatus::Done;

    results
}
