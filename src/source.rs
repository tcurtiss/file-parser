use std::{
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::Result;

/// Where the input file comes from.
pub enum Source {
    File(PathBuf),
    Url(String),
}

impl Source {
    /// Parse a raw CLI argument into a Source.
    /// Anything with a recognised URL scheme is treated as a URL;
    /// everything else is treated as a filesystem path.
    pub fn parse(input: &str) -> Self {
        if is_url(input) {
            Source::Url(input.to_owned())
        } else {
            Source::File(PathBuf::from(input))
        }
    }

    pub fn as_path(&self) -> Option<&Path> {
        match self {
            Source::File(p) => Some(p),
            Source::Url(_)  => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Source::File(p) => p.display().to_string(),
            Source::Url(u)  => u.clone(),
        }
    }

    /// Short label shown in the TUI / GUI transfer row.
    pub fn transfer_label(&self) -> &'static str {
        match self {
            Source::File(_) => "Network transfer",
            Source::Url(_)  => "HTTP download",
        }
    }

    /// Open the source for sequential reading.
    /// Returns the reader and, when known, the total byte count.
    /// Called from inside the pipeline thread — I/O errors surface as Err.
    pub fn open_reader(self) -> Result<(Box<dyn Read + Send>, Option<u64>)> {
        match self {
            Source::File(path) => {
                let file = std::fs::File::open(&path)?;
                let size = file.metadata()?.len();
                Ok((Box::new(file), Some(size)))
            }
            Source::Url(url) => {
                let response = ureq::get(&url)
                    .call()
                    .map_err(|e| anyhow::anyhow!("HTTP error: {e}"))?;
                let content_length = response
                    .header("content-length")
                    .and_then(|v| v.parse::<u64>().ok());
                Ok((response.into_reader(), content_length))
            }
        }
    }
}

/// Returns true for strings that start with a recognised URL scheme.
/// Deliberately narrow: only schemes we can actually fetch.
/// UNC paths (\\server\share or //server/share) do NOT match.
fn is_url(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("ftp://")
        || s.starts_with("ftps://")
}

#[cfg(test)]
mod tests {
    use super::is_url;

    #[test]
    fn urls_are_detected() {
        assert!(is_url("http://example.com/file.txt"));
        assert!(is_url("https://example.com/file.txt"));
        assert!(is_url("ftp://files.example.com/data"));
        assert!(is_url("HTTPS://EXAMPLE.COM/FILE"));   // case-insensitive
    }

    #[test]
    fn paths_are_not_urls() {
        assert!(!is_url("/tmp/file.txt"));
        assert!(!is_url("C:\\data\\file.txt"));
        assert!(!is_url("\\\\server\\share\\file.txt")); // UNC
        assert!(!is_url("//server/share/file.txt"));     // Unix UNC-style
        assert!(!is_url("relative/path/file.txt"));
    }
}
