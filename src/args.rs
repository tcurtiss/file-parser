use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "file-parser", about = "High-performance large file parser")]
pub struct Args {
    /// Path to the file to parse
    pub file: PathBuf,

    /// Enable GUI mode
    #[arg(long, conflicts_with = "quiet")]
    pub gui: bool,

    /// Suppress TUI progress indicators; run silently until complete
    #[arg(long, short = 'q', conflicts_with = "gui")]
    pub quiet: bool,

    /// Number of worker threads (defaults to available CPU count)
    #[arg(long, short)]
    pub workers: Option<usize>,

    /// Force local file strategy, skipping remote detection
    #[arg(long, conflicts_with = "force_remote")]
    pub force_local: bool,

    /// Force remote file strategy, skipping remote detection
    #[arg(long, conflicts_with = "force_local")]
    pub force_remote: bool,
}
