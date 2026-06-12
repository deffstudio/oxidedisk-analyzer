# OxideDisk Analyzer

A fast, single-binary disk-usage analyzer for Windows 11, written in Rust with an
[`egui`](https://github.com/emilk/egui) GUI. Point it at a drive or folder and it walks the tree
on a background thread, then lets you explore every file in a virtualized, sortable, filterable
table alongside a live disk-capacity dashboard.

> **Status:** vertical slice. Scanning, the dashboard, and the file table work today. Duplicate
> detection, junk cleanup, and Administrator elevation are planned (see [Roadmap](#roadmap)).

## Features

- **Multi-threaded scan** — directory walking via [`jwalk`](https://crates.io/crates/jwalk); the
  UI never blocks and updates live as files are discovered.
- **Crash-proof** — permission-denied and vanished-file errors are collected into a scan log, never
  panicking the app.
- **Disk dashboard** — used/free capacity bar for the scanned drive (via `sysinfo`).
- **Virtualized table** — handles millions of files; only visible rows are rendered.
- **Sort & filter** — click column headers to sort by name/size/modified; quick filters for
  *Large Files (>500 MB)*, *Top 100 Largest*, *Stale (>90 days)*, plus category and size sliders.

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
4. Toggle the **Log** checkbox to inspect any files that couldn't be read.

## Architecture

Single-threaded egui UI plus a background scan thread, communicating over an `mpsc` channel.

| File | Responsibility |
| --- | --- |
| `src/models.rs` | Shared types: `FileMetadata`, `DiskInfo`, `Category`, `FilterState`, `ScanMessage` |
| `src/scanner.rs` | Background `jwalk` walk streaming `ScanMessage`s to the UI |
| `src/analyzer.rs` | Filter/sort over indices into the master file list (rayon) |
| `src/ui/` | egui panels: `dashboard`, `filter_bar`, `file_table` + formatters |
| `src/main.rs` | `App` state, channel pumping, panel orchestration |

The master `Vec<FileMetadata>` is never mutated after a scan; a derived `Vec<usize>` (the *view*)
holds the filtered/sorted indices the table renders. See [`CLAUDE.md`](CLAUDE.md) for deeper notes.

## Roadmap

- **Duplicate detection** — group by size, then confirm with `blake3` (first 16 KB, then full hash).
- **Cleanup** — move to Recycle Bin (`trash`) and target Windows Temp / Update cache, with a
  hardcoded System32 blacklist and a dry-run confirmation modal.
- **Privilege escalation** — relaunch elevated on demand only when cleanup runs (no launch-time UAC).
