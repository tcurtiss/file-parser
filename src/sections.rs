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
        name: "METADATA",
        header_pattern: r"^=== METADATA ===",
        content_patterns: &[
            ("server",    r"server:\s*(\S+)"),
            ("timestamp", r"timestamp:\s*(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})"),
            ("version",   r"version:\s*(\S+)"),
        ],
    },
    SectionDef {
        name: "RECORDS",
        header_pattern: r"^=== RECORDS ===",
        content_patterns: &[
            ("date",  r"(\d{4}-\d{2}-\d{2})"),
            ("ip",    r"(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})"),
            ("value", r"value=(\S+)"),
        ],
    },
    SectionDef {
        name: "EVENTS",
        header_pattern: r"^=== EVENTS ===",
        content_patterns: &[
            ("level",   r"(INFO|WARN|ERROR|DEBUG)"),
            ("message", r#"msg="([^"]+)""#),
            ("code",    r"code=(\d+)"),
        ],
    },
];
