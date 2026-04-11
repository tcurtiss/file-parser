use regex::bytes::RegexBuilder;

use crate::sections::SECTIONS;

/// A located section within the file, identified during pass 1.
#[derive(Debug, Clone)]
pub struct SectionBoundary {
    /// Index into `SECTIONS`
    pub section_idx: usize,
    pub name:        String,
    /// Byte offset of the first content byte (after the header line)
    pub start:       u64,
    /// Byte offset past the last content byte (exclusive)
    pub end:         u64,
    /// 1-based line number of the first content line (after the header line)
    pub line_start:  u64,
}

/// Scan `data` for section boundaries defined in `SECTIONS`.
///
/// This is a single-threaded linear pass — fast and cache-friendly.
/// Returns boundaries in order of appearance.
pub fn scan_boundaries(data: &[u8]) -> Vec<SectionBoundary> {
    // Compile one regex per section header pattern
    let header_res: Vec<_> = SECTIONS
        .iter()
        .map(|s| {
            RegexBuilder::new(s.header_pattern)
                .multi_line(true)
                .build()
                .expect("invalid header_pattern")
        })
        .collect();

    // Collect all header hits: (byte_offset_of_match_start, byte_offset_after_line, section_idx)
    let mut hits: Vec<(usize, usize, usize)> = Vec::new();
    for (idx, re) in header_res.iter().enumerate() {
        for m in re.find_iter(data) {
            // The content starts after the newline that ends the header line
            let line_end = data[m.end()..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|p| m.end() + p + 1)
                .unwrap_or(data.len());
            hits.push((m.start(), line_end, idx));
        }
    }

    // Sort by position in file; stable so earlier SECTIONS entry wins ties
    hits.sort_by_key(|&(start, _, _)| start);

    // Build SectionBoundary list — each section ends where the next one starts.
    // Track newlines with a running counter so line_start is O(n) overall.
    let mut boundaries: Vec<SectionBoundary> = Vec::with_capacity(hits.len());
    let mut newlines_seen: u64 = 0;
    let mut scan_pos:      usize = 0;

    for i in 0..hits.len() {
        let (_, content_start, section_idx) = hits[i];
        let content_end = if i + 1 < hits.len() {
            hits[i + 1].0 // next header's match start
        } else {
            data.len()
        };

        // Count newlines from where we last stopped up to this content start
        newlines_seen += data[scan_pos..content_start]
            .iter()
            .filter(|&&b| b == b'\n')
            .count() as u64;
        scan_pos = content_start;

        boundaries.push(SectionBoundary {
            section_idx,
            name:       SECTIONS[section_idx].name.to_string(),
            start:      content_start as u64,
            end:        content_end   as u64,
            line_start: newlines_seen + 1, // 1-based
        });
    }

    boundaries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_one_cat_and_one_dog() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let b = scan_boundaries(data);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].name, "CAT");
        assert_eq!(b[1].name, "DOG");
    }

    #[test]
    fn no_headers_returns_empty() {
        let data = include_bytes!("../tests/fixtures/no_sections.txt");
        let b = scan_boundaries(data);
        assert!(b.is_empty());
    }

    #[test]
    fn preamble_not_captured_in_section() {
        let data = include_bytes!("../tests/fixtures/preamble.txt");
        let b = scan_boundaries(data);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].name, "CAT");
        // The CAT section content must start after the header line, not at byte 0
        assert!(b[0].start > 0);
    }

    #[test]
    fn multiple_boundaries_ordered_by_position() {
        let data = include_bytes!("../tests/fixtures/multi_boundary.txt");
        let b = scan_boundaries(data);
        assert_eq!(b.len(), 4);
        assert_eq!(b[0].name, "CAT");
        assert_eq!(b[1].name, "DOG");
        assert_eq!(b[2].name, "CAT");
        assert_eq!(b[3].name, "DOG");
        // Offsets must be strictly increasing
        for w in b.windows(2) {
            assert!(w[0].start < w[1].start);
        }
    }

    #[test]
    fn sections_do_not_overlap() {
        // w[0].end points at the next header's match start; w[1].start is after
        // that header line — so end < start (header bytes sit between them).
        let data = include_bytes!("../tests/fixtures/multi_boundary.txt");
        let b = scan_boundaries(data);
        for w in b.windows(2) {
            assert!(w[0].end <= w[1].start, "sections overlap: end={} start={}", w[0].end, w[1].start);
        }
    }

    #[test]
    fn last_section_end_equals_file_length() {
        let data = include_bytes!("../tests/fixtures/one_of_each.txt");
        let b = scan_boundaries(data);
        assert_eq!(b.last().unwrap().end, data.len() as u64);
    }
}
