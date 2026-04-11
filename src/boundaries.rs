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

    // Build SectionBoundary list — each section ends where the next one starts
    let mut boundaries: Vec<SectionBoundary> = Vec::with_capacity(hits.len());
    for i in 0..hits.len() {
        let (_, content_start, section_idx) = hits[i];
        let content_end = if i + 1 < hits.len() {
            hits[i + 1].0 // next header's match start
        } else {
            data.len()
        };
        boundaries.push(SectionBoundary {
            section_idx,
            name:  SECTIONS[section_idx].name.to_string(),
            start: content_start as u64,
            end:   content_end   as u64,
        });
    }

    boundaries
}
