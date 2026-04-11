# file-parser

High-performance Rust CLI for parsing large (2GB+) structured text files,
including files on network storage (NFS, SMB, CIFS).

## Architecture

Two-pass pipeline, dispatched based on storage type:

```
Pass 1 (single thread)   linear scan → section boundaries
Pass 2 (rayon pool)      parallel regex parse per section
```

**Local files:** mmap + rayon  
**Remote files:** 16MB-chunk streaming copy to temp → scan behind writer → parallel parse

## Adding / removing sections

Edit `src/sections.rs` only — add or remove entries from the `SECTIONS` array.
Each `SectionDef` has a `header_pattern` (marks start of section) and
`content_patterns` (named regex patterns to match within the section).

## Key modules

| File | Purpose |
|------|---------|
| `sections.rs` | Section definitions — the only file to edit for new sections |
| `storage.rs` | `is_remote()` via `fstatfs` — Linux/macOS/Windows |
| `boundaries.rs` | Pass 1: `scan_boundaries()` — **TODO: implement** |
| `patterns.rs` | `compile_all()` — **TODO: vectorscan** |
| `worker.rs` | `parse_section()` — **TODO: vectorscan `hs_scan()`** |
| `pipeline/local.rs` | mmap → boundaries → rayon |
| `pipeline/remote.rs` | stream copy → scan-behind → workers |
| `tui.rs` | indicatif multi-bar TUI |
| `gui.rs` | egui window (`--gui` flag) |

## CLI flags

```
file-parser <FILE> [--gui] [--workers N] [--force-local] [--force-remote]
```

## Dependency notes

- `vectorscan` crate is commented out in `Cargo.toml` — uncomment when implementing pattern matching
- `egui`/`eframe` 0.28 — immediate mode, no system GUI deps required
- `rayon` parallel iterator used in `pipeline/local.rs`

## Next steps

1. `boundaries.rs` — implement `scan_boundaries()` using header patterns
2. `patterns.rs` — add vectorscan, implement `compile_all()`
3. `worker.rs` — implement `parse_section()` with `hs_scan()`
4. `pipeline/remote.rs` — wire scan-behind loop to real boundary detection
