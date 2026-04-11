use std::sync::Arc;

use crate::{
    boundaries::SectionBoundary,
    patterns::CompiledSection,
    sections::SECTIONS,
    state::{AppState, ParseResult, WorkerState, WorkerStatus},
};

/// Parse a single section of the file, applying all compiled patterns for that section type.
/// Registers a `WorkerState` in `state`, updates progress as it runs, and returns all matches.
///
/// Designed to be called from a rayon thread pool — takes shared references only.
///
/// TODO: implement using vectorscan hs_scan() against the section byte slice.
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

    let _ = (section_def, compiled);

    // TODO:
    // 1. call hs_scan() on section_data with compiled.database
    // 2. in the match callback, create ParseResult for each hit
    // 3. update worker.bytes_done and worker.matches incrementally
    // 4. push results into a local Vec and return it

    *worker.status.lock().unwrap() = WorkerStatus::Done;

    Vec::new()
}
