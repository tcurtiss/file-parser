# file-parser

High-performance CLI tool for parsing large (2 GB+) structured text files. Supports local files, network-mounted filesystems, and URLs. Features a terminal progress UI and an optional native GUI.

## Features

- **Two-pass parallel parsing** — single-threaded boundary scan (pass 1) feeds a rayon worker pool for concurrent section parsing (pass 2)
- **Zero-copy local reads** — `mmap` for local files; large buffered reads for remote/network sources
- **Automatic local/remote detection** — uses `fstatfs` on Linux/macOS and `GetFileInformationByHandleEx` on Windows to detect NFS, SMB, CIFS, FUSE, and AFS mounts
- **URL support** — accepts `http://`, `https://`, `ftp://`, and `ftps://` sources; streams to a local temp file while scanning behind the writer
- **TUI progress** — per-section progress bars with byte counts, match counts, and transfer speed via `indicatif`
- **Optional GUI** — native window with transfer and worker progress bars via `egui`/`eframe` (`--gui`)
- **Configurable sections** — add or remove parsed sections by editing a single registry in `src/sections.rs`

## Usage

```
file-parser [OPTIONS] <FILE>
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<FILE>` | File to parse — filesystem path or URL (`http://`, `https://`, `ftp://`, `ftps://`) |

### Options

| Option | Description |
|--------|-------------|
| `--gui` | Open a native GUI window instead of TUI output |
| `-q`, `--quiet` | Suppress all progress output; run silently |
| `-w`, `--workers <N>` | Number of worker threads (default: available CPU count) |
| `--force-local` | Skip remote detection; treat file as local |
| `--force-remote` | Skip remote detection; treat file as remote |

### Examples

```bash
# Parse a local file with TUI progress
file-parser /data/large-log.txt

# Parse a network-mounted file with GUI
file-parser --gui /mnt/nas/archive.txt

# Download and parse a remote file silently
file-parser --quiet https://example.com/data.txt

# Use 8 worker threads
file-parser --workers 8 /data/large-log.txt
```

## Results

Each parsed section boundary produces one `ParseResult` per content pattern. Every result carries:

| Field | Description |
|-------|-------------|
| `section` | Section name (e.g. `CAT`, `DOG`) |
| `label` | Content pattern label (e.g. `value`, `events`) |
| `offset` | Byte offset of the section's first content byte within the file — i.e. the position immediately after the header line's newline. All results from the same boundary share this value. |
| `line` | 1-based line number of the section's first content line — the line immediately after the header. All results from the same boundary share this value. |
| `value` | String produced by the pattern's handler (e.g. a sum, count, or comma-joined list) |

Both `offset` and `line` identify **where in the file the section starts**, not where individual matches occur within it. Two boundaries of the same section type (e.g. two `CAT` sections) will always have different `offset` and `line` values.

**Example** — a file whose second section starts at byte 4096 on line 83:

```
[CAT       ] value        @       4096  line      83  10188918
[CAT       ] events       @       4096  line      83  988
[CAT       ] host         @       4096  line      83  gateway.edge
[CAT       ] tags         @       4096  line      83  red, blue, green
```

## Adding or Removing Sections

All section configuration lives in `src/sections.rs`. To add a section, append a `SectionDef` to the `SECTIONS` array. To remove one, delete its entry. Order in the array determines priority when a line matches multiple headers.

### Section structure

```rust
SectionDef {
    name:             "BIRD",
    header_pattern:   r"^Bird Boundary \d+",
    content_patterns: &[ /* one or more ContentPattern entries */ ],
    finalizer:        finalizers::identity,
}
```

| Field | Type | Purpose |
|-------|------|---------|
| `name` | `&str` | Identifier shown in progress display and results |
| `header_pattern` | `&str` (regex) | Matched against each line to detect where the section starts; supports `^` anchors |
| `content_patterns` | `&[ContentPattern]` | Patterns to match within the section (see below) |
| `finalizer` | `fn(Vec<ParseResult>) -> Vec<ParseResult>` | Post-processes all pattern results for this section boundary |

### Content patterns

Each `ContentPattern` defines what to match and how to aggregate the matches:

```rust
ContentPattern {
    label:   "total",          // shown in results
    regex:   r"AddVal (\d+)", // capture group 1 is passed to the handler
    handler: handlers::sum,   // aggregation function
}
```

The handler is called once per section boundary with a slice of all captures (group 1 if the pattern has one, full match otherwise). It returns a single `String` value for the result.

#### Built-in handlers (`handlers::*`)

| Handler | Behaviour |
|---------|-----------|
| `handlers::sum` | Parse captures as `u64` and return the sum |
| `handlers::count` | Return the number of matches as a string |
| `handlers::first` | Return the first capture verbatim; empty string if no matches |
| `handlers::collect` | Join all captures with `", "` |

#### Custom handler

Any `fn(&[&[u8]]) -> String` function can be used as a handler:

```rust
fn sum_hex(captures: &[&[u8]]) -> String {
    captures.iter()
        .filter_map(|c| std::str::from_utf8(c).ok())
        .filter_map(|s| u64::from_str_radix(s, 16).ok())
        .sum::<u64>()
        .to_string()
}

ContentPattern { label: "hex_total", regex: r"0x([0-9a-fA-F]+)", handler: sum_hex }
```

### Finalizers

A finalizer receives the `Vec<ParseResult>` produced by all content patterns for one section boundary and may transform, filter, or augment them before they are stored.

#### Built-in finalizer

`finalizers::identity` — returns results unchanged. Use this when no post-processing is needed.

#### Custom finalizer

Any `fn(Vec<ParseResult>) -> Vec<ParseResult>` function can be used:

```rust
use crate::state::ParseResult;

// Suppress any result whose value is "0"
fn drop_zeros(mut results: Vec<ParseResult>) -> Vec<ParseResult> {
    results.retain(|r| r.value != "0");
    results
}

// Append a grand-total row summing all numeric results
fn add_grand_total(mut results: Vec<ParseResult>) -> Vec<ParseResult> {
    let total: u64 = results.iter()
        .filter_map(|r| r.value.parse::<u64>().ok())
        .sum();
    results.push(ParseResult {
        section: results[0].section.clone(),
        label:   "grand_total".into(),
        offset:  results[0].offset,
        value:   total.to_string(),
    });
    results
}
```

Then reference it in `SECTIONS`:

```rust
SectionDef {
    name:             "BIRD",
    header_pattern:   r"^Bird Boundary \d+",
    content_patterns: &[
        ContentPattern { label: "total",  regex: r"AddVal (\d+)",       handler: handlers::sum     },
        ContentPattern { label: "events", regex: r"^Event \w+",         handler: handlers::count   },
        ContentPattern { label: "host",   regex: r"Host: (\S+)",        handler: handlers::first   },
        ContentPattern { label: "tags",   regex: r"Tag=(\w+)",          handler: handlers::collect },
        ContentPattern { label: "hex",    regex: r"0x([0-9a-fA-F]+)",   handler: sum_hex           },
    ],
    finalizer: add_grand_total,
}
```

## Building

Requires Rust 1.75+ and Cargo.

```bash
# Debug build
cargo build

# Optimised release build (LTO, size-optimised)
cargo build --release
```

## Testing

```bash
# Run the full test suite
cargo test

# Run only boundary-detection tests
cargo test boundaries

# Run only parse/accumulation tests
cargo test worker

# Run a single test by name
cargo test preamble_addval_not_counted

# Show stdout from passing tests (useful when debugging)
cargo test -- --nocapture
```

### Test structure

| Location | What is tested |
|----------|----------------|
| `src/boundaries.rs` | `scan_boundaries()` — header detection, ordering, section offsets |
| `src/worker.rs` | Full pipeline — AddVal accumulation, zero-sum, preamble exclusion, multi-boundary independence |
| `src/source.rs` | URL vs path detection |

### Fixture files

Test inputs live in `tests/fixtures/`. To add a new test scenario, drop a `.txt` file there and reference it in the relevant `#[cfg(test)]` block with:

```rust
let data = include_bytes!("../tests/fixtures/your_file.txt");
```

| Fixture | Purpose |
|---------|---------|
| `one_of_each.txt` | One CAT + one DOG section with known sums |
| `multi_boundary.txt` | Two CATs + two DOGs, each summed independently |
| `no_addval.txt` | Sections with no matching lines — expects zero sums |
| `no_sections.txt` | No section headers — expects empty result set |
| `preamble.txt` | AddVal lines before first header — must not be counted |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `memmap2` | Zero-copy mmap for local files |
| `rayon` | Parallel section worker pool |
| `regex` | Header and content pattern matching |
| `indicatif` | Terminal progress bars |
| `egui` / `eframe` | Optional native GUI |
| `ureq` | HTTP/FTP streaming for URL sources |
| `libc` | `fstatfs` for remote filesystem detection |
| `anyhow` | Error handling |
| `ctrlc` | Clean Ctrl-C handling |

## Platform Support

| Platform | Local files | Network mounts | GUI |
|----------|------------|----------------|-----|
| Linux | Yes | NFS, SMB, CIFS, FUSE, AFS | Yes |
| macOS | Yes | NFS, SMB, AFP, FUSE | Yes |
| Windows | Yes | SMB/CIFS, remote shares | Yes |
