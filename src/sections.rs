use crate::state::ParseResult;

/// A single content pattern within a section, including how its captures are aggregated.
pub struct ContentPattern {
    /// Label shown in results and progress display
    pub label:   &'static str,
    /// Regex matched against section content lines
    pub regex:   &'static str,
    /// Called once per section with all captures from this pattern.
    /// Receives group 1 of each match; falls back to the full match when the
    /// pattern has no capture group. Returns the final value string.
    pub handler: fn(&[&[u8]]) -> String,
}

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
    /// Content patterns to match within this section
    pub content_patterns: &'static [ContentPattern],
    /// Called after all content patterns are processed. May transform, filter, or
    /// augment the per-pattern results. Use `finalizers::identity` to pass through unchanged.
    pub finalizer: fn(Vec<ParseResult>) -> Vec<ParseResult>,
}

/// Built-in handlers for common aggregation patterns.
///
/// A handler receives all captures from one content pattern across the entire section
/// and returns a single value string for the result.
pub mod handlers {
    /// Parse each capture as `u64` and return the sum. Non-numeric captures are skipped.
    pub fn sum(captures: &[&[u8]]) -> String {
        captures
            .iter()
            .filter_map(|c| std::str::from_utf8(c).ok())
            .filter_map(|s| s.parse::<u64>().ok())
            .sum::<u64>()
            .to_string()
    }

    /// Return the number of matches as a string.
    pub fn count(captures: &[&[u8]]) -> String {
        captures.len().to_string()
    }

    /// Return the first capture verbatim, or an empty string if there were no matches.
    pub fn first(captures: &[&[u8]]) -> String {
        captures
            .first()
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .unwrap_or_default()
    }

    /// Join all captures with `", "`.
    pub fn collect(captures: &[&[u8]]) -> String {
        captures
            .iter()
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Built-in section finalizers.
///
/// A finalizer receives the `Vec<ParseResult>` produced by all content patterns
/// for one section boundary and may transform, filter, or augment them.
pub mod finalizers {
    use crate::state::ParseResult;

    /// No-op: return results unchanged.
    pub fn identity(results: Vec<ParseResult>) -> Vec<ParseResult> {
        results
    }
}

/// Registry of all sections to parse.
///
/// Add or remove entries here to change what the parser looks for.
pub const SECTIONS: &[SectionDef] = &[
    SectionDef {
        name: "CAT",
        header_pattern: r"^Cat Boundary \d+",
        content_patterns: &[
            ContentPattern { label: "value",  regex: r"AddVal (\d+)", handler: handlers::sum     },
            ContentPattern { label: "events", regex: r"^Event \w+",   handler: handlers::count   },
            ContentPattern { label: "host",   regex: r"Host: (\S+)",  handler: handlers::first   },
            ContentPattern { label: "tags",   regex: r"Tag=(\w+)",    handler: handlers::collect },
        ],
        finalizer: finalizers::identity,
    },
    SectionDef {
        name: "DOG",
        header_pattern: r"^Dog Boundary \d+",
        content_patterns: &[
            ContentPattern {
                label:   "value",
                regex:   r"AddVal (\d+)",
                handler: handlers::sum,
            },
        ],
        finalizer: finalizers::identity,
    },
];
