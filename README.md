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

## Adding or Removing Sections

Open `src/sections.rs` and edit the `SECTIONS` array. Each entry defines:

- `name` — identifier shown in progress and results
- `header_pattern` — regex matched against each line to detect the section start
- `content_patterns` — list of `(label, regex)` pairs to match within the section; capture group 1 is extracted as the value

```rust
SectionDef {
    name: "CAT",
    header_pattern: r"^Cat Boundary \d+",
    content_patterns: &[
        ("value", r"AddVal (\d+)"),
    ],
},
```

For each content pattern, all capture group 1 matches within the section are parsed as integers and summed. The total is reported as the result for that label when the section completes.

## Building

Requires Rust 1.75+ and Cargo.

```bash
# Debug build
cargo build

# Optimised release build (LTO, size-optimised)
cargo release
```

### Dependencies

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
