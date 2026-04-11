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
///
/// TODO: implement using vectorscan for multi-pattern single-pass scanning.
///       For now returns an empty vec (no sections found).
pub fn scan_boundaries(data: &[u8]) -> Vec<SectionBoundary> {
    let _ = (data, SECTIONS);
    // TODO:
    // 1. compile header_pattern for each SectionDef
    // 2. scan data linearly, match header patterns
    // 3. when a new header is found, close the previous section
    // 4. return completed boundaries
    Vec::new()
}
