use regex::bytes::{Regex, RegexBuilder};

use crate::sections::{SectionDef, SECTIONS};

/// A compiled content pattern, ready to scan against raw bytes.
pub struct CompiledPattern {
    pub label:   String,
    pub regex:   Regex,
    /// Handler copied from `ContentPattern` — called with all captures after scanning.
    pub handler: fn(&[&[u8]]) -> String,
}

/// Compiled patterns for a single section, ready to scan against raw bytes.
pub struct CompiledSection {
    pub patterns: Vec<CompiledPattern>,
}

/// Compile content patterns for all sections upfront so workers share read-only data.
pub fn compile_all() -> anyhow::Result<Vec<CompiledSection>> {
    SECTIONS.iter().map(compile_section).collect()
}

fn compile_section(section: &SectionDef) -> anyhow::Result<CompiledSection> {
    let patterns = section
        .content_patterns
        .iter()
        .map(|cp| {
            let re = RegexBuilder::new(cp.regex)
                .multi_line(true)
                .build()
                .map_err(|e| anyhow::anyhow!("bad pattern {:?}: {e}", cp.regex))?;
            Ok(CompiledPattern {
                label:   cp.label.to_string(),
                regex:   re,
                handler: cp.handler,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(CompiledSection { patterns })
}
