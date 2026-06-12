# OxideDisk Analyzer

A fast, single-binary disk-usage analyzer for Windows 11, written in Rust with an
[`egui`](https://github.com/emilk/egui) GUI. Point it at a drive or folder and it walks the tree
on a background thread, then lets you explore every file in a virtualized, sortable, filterable
table alongside a live disk-capacity dashboard.

> **Status:** scanning, the dashboard, the file table, and duplicate detection work today. Junk
> cleanup and Administrator elevation are planned (see [Roadmap](#roadmap)).

## Features

- **Multi-threaded scan** — directory walking via [`jwalk`](https://crates.io/crates/jwalk); the
  UI never blocks and updates live as files are discovered.
- **Crash-proof** — permission-denied and vanished-file errors are collected into a scan log, never
  panicking the app.
- **Disk dashboard** — used/free capacity bar for the scanned drive (via `sysinfo`).
- **Virtualized table** — handles millions of files; only visible rows are rendered.
- **Sort & filter** — click column headers to sort by name/size/modified; quick filters for
  *Large Files (>500 MB)*, *Top 100 Largest*, *Stale (>90 days)*, plus category and size sliders.
- **Duplicate detection** — [`blake3`](https://crates.io/crates/blake3) content hashing with a
  size → 16 KB-prefix → full-hash funnel, run on a background thread. Results group by content with
  per-set and total reclaimable-space figures.

## Requirements

- **Rust 1.85+** (the dependency tree uses edition-2024 crates). Update with `rustup update stable`.
- **Windows** with the MSVC toolchain.

## Build & run

```bash
cargo run              # build + launch the GUI
cargo build --release  # optimized single exe at target/release/oxidedisk-analyzer.exe
```

In release builds the console window is hidden; debug builds keep it for logging.

## Usage

1. Click **📁 Select folder** and choose a drive root (e.g. `C:\`) or any directory.
2. Watch the dashboard fill in as the scan streams results.
3. Use the left **Filters** panel and click table headers to drill into the data.
4. Click **🔍 Find Duplicates**, then switch to the **Duplicates** view to browse content-identical
   sets and how much space they waste.
5. Toggle the **Log** checkbox to inspect any files that couldn't be read.

## Architecture

Single-threaded egui UI plus background worker threads (scan, duplicate-finder), each communicating
over its own `mpsc` channel.

| File | Responsibility |
| --- | --- |
| `src/models.rs` | Shared types: `FileMetadata`, `DiskInfo`, `Category`, `FilterState`, `ScanMessage`, `DuplicateGroup`, `DupMessage` |
| `src/scanner.rs` | Background `jwalk` walk streaming `ScanMessage`s to the UI |
| `src/analyzer.rs` | Filter/sort over indices into the master file list (rayon) |
| `src/duplicates.rs` | Background blake3 dedup funnel (size → prefix → full hash) streaming `DupMessage`s |
| `src/ui/` | egui panels: `dashboard`, `filter_bar`, `file_table`, `duplicates` + formatters |
| `src/main.rs` | `App` state, channel pumping, panel orchestration |

The master file list is held in an `Arc<Vec<FileMetadata>>` (shared cheaply with worker threads) and
never mutated after a scan; a derived `Vec<usize>` (the *view*) holds the filtered/sorted indices the
table renders. See [`CLAUDE.md`](CLAUDE.md) for deeper notes.

## Roadmap

- **Cleanup** — move to Recycle Bin (`trash`) and target Windows Temp / Update cache, with a
  hardcoded System32 blacklist and a dry-run confirmation modal.
- **Privilege escalation** — relaunch elevated on demand only when cleanup runs (no launch-time UAC).
