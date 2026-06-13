# OxideDisk Analyzer

A fast, single-binary disk-usage analyzer for Windows 11, written in Rust with an
[`egui`](https://github.com/emilk/egui) GUI. Point it at a drive or folder and it walks the tree
on a background thread, then lets you explore every file in a virtualized, sortable, filterable
table alongside a live disk-capacity dashboard.

> **Status:** the vertical slice is complete — scanning, the dashboard, the file table, duplicate
> detection, temp-folder cleanup (move to Recycle Bin), and on-demand Administrator elevation all
> work today. See [Roadmap](#roadmap) for what's next.

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
- **Temp cleanup** — scans known Windows junk locations (User/System Temp, Update cache, Prefetch,
  thumbnail and browser caches) and moves selected files to the Recycle Bin via
  [`trash`](https://crates.io/crates/trash) — recoverable, never a hard delete.
- **On-demand elevation** — runs unprivileged; when cleaning protected system folders needs
  Administrator rights, a **🛡 Run as Administrator** button relaunches the binary elevated (single
  UAC prompt) straight into the cleanup view. No launch-time admin manifest.

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
5. Click **🧹 Temp Cleanup** to scan known junk folders; review the list and **🗑 Clean All** to move
   them to the Recycle Bin. If protected system folders need admin rights, click
   **🛡 Run as Administrator** to relaunch elevated.
6. Toggle the **Log** checkbox to inspect any files that couldn't be read.

## Architecture

Single-threaded egui UI plus background worker threads (scan, duplicate-finder), each communicating
over its own `mpsc` channel.

| File | Responsibility |
| --- | --- |
| `src/models.rs` | Shared types: `FileMetadata`, `DiskInfo`, `Category`, `FilterState`, `ScanMessage`, `DuplicateGroup`, `DupMessage` |
| `src/scanner.rs` | Background `jwalk` walk streaming `ScanMessage`s to the UI |
| `src/analyzer.rs` | Filter/sort over indices into the master file list (rayon) |
| `src/duplicates.rs` | Background blake3 dedup funnel (size → prefix → full hash) streaming `DupMessage`s |
| `src/cleanup.rs` | Known temp-folder discovery + move-to-Recycle-Bin (`trash`) |
| `src/elevation.rs` | UAC: detect admin token, relaunch elevated on demand (`windows-sys`, `ShellExecuteW` `runas`) |
| `src/ui/` | egui panels: `dashboard`, `filter_bar`, `file_table`, `duplicates`, `cleanup` + formatters |
| `src/main.rs` | `App` state, channel pumping, panel orchestration |

The master file list is held in an `Arc<Vec<FileMetadata>>` (shared cheaply with worker threads) and
never mutated after a scan; a derived `Vec<usize>` (the *view*) holds the filtered/sorted indices the
table renders. See [`CLAUDE.md`](CLAUDE.md) for deeper notes.

## Roadmap

The core slice (scan, dashboard, table, duplicates, cleanup, elevation) is complete. Possible next
steps:

- **Safer cleanup** — a dry-run confirmation modal and a hardcoded System32/critical-path blacklist
  before any delete.
- **Persisted settings** — remember the last scanned root, window size, and filter presets.
- **Tree map view** — a visual size breakdown by folder alongside the flat table.
