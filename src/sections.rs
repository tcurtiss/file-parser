/// Defines one parseable section within the input file.
///
/// To add a section:    add a new `SectionDef` entry to `SECTIONS`.
/// To remove a section: delete its entry from `SECTIONS`.
/// Order in `SECTIONS` determines priority when a line matches multiple headers.
pub struct SectionDef {
    /// Unique section identifier used in results and progress display
    pub name: &'static str,
    /// Regex pattern matched against each line to detect the section header
    pub header_pattern: &'static str,
    /// Named content patterns to match within this section: (label, regex)
    pub content_patterns: &'static [(&'static str, &'static str)],
}

/// Registry of all sections to parse.
///
/// Add or remove entries here to change what the parser looks for.
pub const SECTIONS: &[SectionDef] = &[
    SectionDef {
        name: "CAT",
        header_pattern: r"^Cat Boundary \d+",
        content_patterns: &[
            ("value", r"AddVal (\d+)"),
        ],
    },
    SectionDef {
        name: "DOG",
        header_pattern: r"^Dog Boundary \d+",
        content_patterns: &[
            ("value", r"AddVal (\d+)"),
        ],
    },
];
