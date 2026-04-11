use crate::sections::{SectionDef, SECTIONS};

/// Compiled patterns for a single section, ready to scan against raw bytes.
///
/// TODO: backed by vectorscan for multi-pattern single-pass matching.
pub struct CompiledSection {
    pub section_idx: usize,
    // TODO: vectorscan database and scratch space per section
}

/// Compile patterns for all sections upfront so workers share read-only databases.
///
/// TODO: implement using vectorscan hs_compile_multi() per section.
pub fn compile_all() -> anyhow::Result<Vec<CompiledSection>> {
    SECTIONS
        .iter()
        .enumerate()
        .map(|(idx, section)| compile_section(idx, section))
        .collect()
}

fn compile_section(section_idx: usize, section: &SectionDef) -> anyhow::Result<CompiledSection> {
    let _ = section;
    // TODO:
    // 1. collect section.content_patterns into pattern + flags arrays
    // 2. call hs_compile_multi() / vectorscan equivalent
    // 3. store resulting database in CompiledSection
    Ok(CompiledSection { section_idx })
}
