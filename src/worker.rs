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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{boundaries::scan_boundaries, patterns::compile_all, state::AppState};

    fn make_state() -> Arc<AppState> {
        Arc::new(AppState::new(0, false, "test", true))
    }

    /// Run the full parse pipeline on raw bytes and return all results.
    fn run_parse(data: &[u8]) -> Vec<crate::state::ParseResult> {
        let boundaries = scan_boundaries(data);
        let compiled   = compile_all().unwrap();
        let state      = make_state();
        boundaries
            .iter()
            .flat_map(|b| parse_section(data, b, &compiled[b.section_idx], &state))
            .collect()
    }

    // ── Basic accumulation ─────────────────────────────────────────────────

    #[test]
    fn cat_section_sums_addval() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let results = run_parse(data);
        let cat = results.iter().find(|r| r.section == "CAT").unwrap();
        assert_eq!(cat.value, "30"); // 10 + 20
    }

    #[test]
    fn dog_section_sums_addval() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let results = run_parse(data);
        let dog = results.iter().find(|r| r.section == "DOG").unwrap();
        assert_eq!(dog.value, "20"); // 5 + 15
    }

    // ── Edge cases ─────────────────────────────────────────────────────────

    #[test]
    fn no_addval_lines_yields_zero_sum() {
        let data = include_bytes!("../tests/fixtures/no_addval.txt");
        let results = run_parse(data);
        assert_eq!(results.len(), 2); // one result per section
        assert!(results.iter().all(|r| r.value == "0"));
    }

    #[test]
    fn no_sections_yields_no_results() {
        let data = include_bytes!("../tests/fixtures/no_sections.txt");
        let results = run_parse(data);
        assert!(results.is_empty());
    }

    #[test]
    fn preamble_addval_not_counted() {
        let data = include_bytes!("../tests/fixtures/preamble.txt");
        let results = run_parse(data);
        assert_eq!(results.len(), 1);
        let cat = results.iter().find(|r| r.section == "CAT").unwrap();
        assert_eq!(cat.value, "42"); // preamble AddVal 999 must be excluded
    }

    // ── Multiple boundaries ────────────────────────────────────────────────

    #[test]
    fn multiple_boundaries_produce_independent_sums() {
        let data = include_bytes!("../tests/fixtures/multi_boundary.txt");
        let results = run_parse(data);
        let cats: Vec<_> = results.iter().filter(|r| r.section == "CAT").collect();
        let dogs: Vec<_> = results.iter().filter(|r| r.section == "DOG").collect();
        assert_eq!(cats.len(), 2);
        assert_eq!(dogs.len(), 2);
        // Each boundary is summed independently, not merged
        let cat_sums: Vec<u64> = cats.iter().map(|r| r.value.parse().unwrap()).collect();
        let dog_sums: Vec<u64> = dogs.iter().map(|r| r.value.parse().unwrap()).collect();
        assert_eq!(cat_sums, vec![100, 200]);
        assert_eq!(dog_sums, vec![50, 75]);
    }

    #[test]
    fn result_label_matches_content_pattern_name() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let results = run_parse(data);
        assert!(results.iter().all(|r| r.label == "value"));
    }
}
