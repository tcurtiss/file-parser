use regex::bytes::Regex;

use crate::sections::{SectionDef, SECTIONS};

/// Compiled patterns for a single section, ready to scan against raw bytes.
pub struct CompiledSection {
    /// (label, compiled regex) for each content pattern in this section
    pub patterns: Vec<(String, Regex)>,
}

/// Compile content patterns for all sections upfront so workers share read-only data.
pub fn compile_all() -> anyhow::Result<Vec<CompiledSection>> {
    SECTIONS.iter().map(compile_section).collect()
}

fn compile_section(section: &SectionDef) -> anyhow::Result<CompiledSection> {
    let patterns = section
        .content_patterns
        .iter()
        .map(|(label, pat)| {
            let re = Regex::new(pat)
                .map_err(|e| anyhow::anyhow!("bad pattern {pat:?}: {e}"))?;
            Ok((label.to_string(), re))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(CompiledSection { patterns })
}
