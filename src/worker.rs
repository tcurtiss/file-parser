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
/// For each content pattern:
///   1. All captures (group 1 if present, full match otherwise) are collected.
///   2. The pattern's `handler` is called with those captures to produce a value string.
///   3. A `ParseResult` is pushed for that pattern.
/// After all patterns are processed, the section's `finalizer` is called on the results.
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

    for compiled_pattern in &compiled.patterns {
        // Collect captures for this pattern across the whole section.
        // Use group 1 when present; fall back to the full match (group 0) so
        // that count/collect handlers work on patterns without a capture group.
        let owned_caps: Vec<Vec<u8>> = compiled_pattern.regex
            .captures_iter(section_data)
            .map(|caps| {
                caps.get(1)
                    .unwrap_or_else(|| caps.get(0).unwrap()) // group 0 always exists
                    .as_bytes()
                    .to_vec()
            })
            .collect();

        worker.matches.fetch_add(owned_caps.len() as u64, Ordering::Relaxed);

        let cap_refs: Vec<&[u8]> = owned_caps.iter().map(Vec::as_slice).collect();
        let value = (compiled_pattern.handler)(&cap_refs);

        results.push(ParseResult {
            section: section_def.name.to_string(),
            label:   compiled_pattern.label.clone(),
            offset:  boundary.start,
            line:    boundary.line_start,
            value,
        });
    }

    // Apply per-section finalizer
    let results = (section_def.finalizer)(results);

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
        // Numeric aggregations (sum, count) must be zero
        assert!(results.iter()
            .filter(|r| r.label == "value" || r.label == "events")
            .all(|r| r.value == "0"));
        // String aggregations (first, collect) must be empty
        assert!(results.iter()
            .filter(|r| r.label == "host" || r.label == "tags")
            .all(|r| r.value.is_empty()));
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
        // One CAT section with four patterns — no DOG sections
        assert!(results.iter().all(|r| r.section == "CAT"));
        let sum = results.iter().find(|r| r.label == "value").unwrap();
        assert_eq!(sum.value, "42"); // preamble AddVal 999 must be excluded
    }

    // ── Multiple boundaries ────────────────────────────────────────────────

    #[test]
    fn multiple_boundaries_produce_independent_sums() {
        let data = include_bytes!("../tests/fixtures/multi_boundary.txt");
        let results = run_parse(data);
        // Filter to the "value" (sum) label to isolate numeric results
        let cat_sums: Vec<u64> = results.iter()
            .filter(|r| r.section == "CAT" && r.label == "value")
            .map(|r| r.value.parse().unwrap())
            .collect();
        let dog_sums: Vec<u64> = results.iter()
            .filter(|r| r.section == "DOG" && r.label == "value")
            .map(|r| r.value.parse().unwrap())
            .collect();
        assert_eq!(cat_sums, vec![100, 200]);
        assert_eq!(dog_sums, vec![50, 75]);
    }

    #[test]
    fn result_labels_match_section_patterns() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let results = run_parse(data);
        // CAT has four patterns; DOG has one
        let cat_labels: std::collections::HashSet<&str> = results.iter()
            .filter(|r| r.section == "CAT")
            .map(|r| r.label.as_str())
            .collect();
        assert_eq!(cat_labels, ["value", "events", "host", "tags"].iter().copied().collect());
        assert!(results.iter()
            .filter(|r| r.section == "DOG")
            .all(|r| r.label == "value"));
    }

    // ── New CAT handler tests ──────────────────────────────────────────────

    #[test]
    fn cat_count_handler_counts_events() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "CAT" && r.label == "events").unwrap();
        assert_eq!(r.value, "2"); // Event alpha, Event beta
    }

    #[test]
    fn cat_first_handler_returns_first_host() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "CAT" && r.label == "host").unwrap();
        assert_eq!(r.value, "server1.example.com"); // first of two Host: lines
    }

    #[test]
    fn cat_collect_handler_joins_tags() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "CAT" && r.label == "tags").unwrap();
        assert_eq!(r.value, "red, blue");
    }

    // ── Large fixture (seed=42, 10 000 lines per section) ─────────────────

    #[test]
    fn large_cat_sum() {
        let data = include_bytes!("../tests/fixtures/large.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "CAT" && r.label == "value").unwrap();
        assert_eq!(r.value, "10188918");
    }

    #[test]
    fn large_cat_event_count() {
        let data = include_bytes!("../tests/fixtures/large.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "CAT" && r.label == "events").unwrap();
        assert_eq!(r.value, "988");
    }

    #[test]
    fn large_cat_first_host() {
        let data = include_bytes!("../tests/fixtures/large.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "CAT" && r.label == "host").unwrap();
        assert_eq!(r.value, "gateway.edge");
    }

    #[test]
    fn large_cat_tag_count() {
        let data = include_bytes!("../tests/fixtures/large.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "CAT" && r.label == "tags").unwrap();
        // 1034 tags joined with ", " — verify count via delimiter
        assert_eq!(r.value.split(", ").count(), 1034);
    }

    #[test]
    fn large_dog_sum() {
        let data = include_bytes!("../tests/fixtures/large.txt");
        let results = run_parse(data);
        let r = results.iter().find(|r| r.section == "DOG" && r.label == "value").unwrap();
        assert_eq!(r.value, "10031203");
    }

    // ── Handler and finalizer mechanics ───────────────────────────────────

    #[test]
    fn count_handler_counts_matches() {
        use crate::sections::{ContentPattern, SectionDef, handlers, finalizers};
        use crate::patterns::CompiledSection;

        // Build a one-off section using the count handler
        let def = SectionDef {
            name: "TEST",
            header_pattern: r"^Test \d+",
            content_patterns: &[
                ContentPattern { label: "hits", regex: r"AddVal \d+", handler: handlers::count },
            ],
            finalizer: finalizers::identity,
        };
        let re = regex::bytes::Regex::new(def.content_patterns[0].regex).unwrap();
        let compiled = CompiledSection {
            patterns: vec![crate::patterns::CompiledPattern {
                label:   "hits".to_string(),
                regex:   re,
                handler: handlers::count,
            }],
        };

        // Fake a boundary covering the entire input
        let data = b"AddVal 10\nAddVal 20\nAddVal 30\n";
        let boundary = crate::boundaries::SectionBoundary {
            section_idx: 0,
            name:        "TEST".to_string(),
            start:       0,
            end:         data.len() as u64,
            line_start:  1,
        };

        // Temporarily override SECTIONS isn't possible with a const, so call
        // the handler directly to verify count behaviour.
        let caps: Vec<Vec<u8>> = compiled.patterns[0].regex
            .captures_iter(data)
            .map(|c| c.get(0).unwrap().as_bytes().to_vec())
            .collect();
        let cap_refs: Vec<&[u8]> = caps.iter().map(Vec::as_slice).collect();
        assert_eq!(handlers::count(&cap_refs), "3");
        let _ = (def, boundary); // suppress unused warnings
    }

    #[test]
    fn collect_handler_joins_captures() {
        use crate::sections::handlers;
        let caps: &[&[u8]] = &[b"alpha", b"beta", b"gamma"];
        assert_eq!(handlers::collect(caps), "alpha, beta, gamma");
    }

    #[test]
    fn first_handler_returns_first_only() {
        use crate::sections::handlers;
        let caps: &[&[u8]] = &[b"first", b"second"];
        assert_eq!(handlers::first(caps), "first");
    }

    #[test]
    fn first_handler_empty_returns_empty_string() {
        use crate::sections::handlers;
        assert_eq!(handlers::first(&[]), "");
    }

    #[test]
    fn finalizer_can_filter_results() {
        use crate::state::ParseResult;

        fn drop_zeros(mut results: Vec<ParseResult>) -> Vec<ParseResult> {
            results.retain(|r| r.value != "0");
            results
        }

        let input = vec![
            ParseResult { section: "X".into(), label: "a".into(), offset: 0, line: 1, value: "0".into()  },
            ParseResult { section: "X".into(), label: "b".into(), offset: 0, line: 1, value: "42".into() },
        ];
        let output = drop_zeros(input);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].label, "b");
    }
}
