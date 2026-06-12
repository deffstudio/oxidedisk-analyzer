# GEMINI.md

This file provides guidance to Gemini when working with code in this repository.

## Project

OxideDisk Analyzer — a single-binary Windows 11 disk-usage analyzer written in Rust with an
`egui`/`eframe` GUI. Current state is a **vertical slice**: scan a folder/drive, show a disk-capacity
dashboard, and explore files in a virtualized, sortable/filterable table. Duplicate detection (blake3),
cleanup (recycle bin / temp folders), and UAC elevation are **deferred** (not yet implemented).

## Commands

```bash
cargo run              # build + launch the GUI (console window kept in debug)
cargo build --release  # single optimized exe (LTO on); console hidden via windows_subsystem
cargo check            # fast type-check during iteration
cargo clippy           # lint
cargo fmt              # format
```

There are no tests yet. When adding them, `cargo test`; a single test: `cargo test <name>`.

## Architecture

The app is **single-threaded UI + a background scan thread**, communicating over an `mpsc` channel.
This is the central design constraint: the egui loop must never block, so all filesystem work happens
off-thread and results stream back as messages.

**Data flow:** `main.rs` (`App`) owns the state and orchestrates everything → `scanner::spawn_scan`
walks the tree on a worker thread, sending `ScanMessage`s → `App::pump_scan` drains them each frame →
on `Done`, `analyzer` filters + sorts → the `ui` panels render.

- `src/models.rs` — shared types: `FileMetadata`, `DiskInfo`, `Category` (extension→category map),
  `SortKey`, `FilterState`, and the `ScanMessage` enum (`Progress` / `Error` / `Done`) that defines
  the thread→UI protocol.
- `src/scanner.rs` — `spawn_scan(root, tx, ctx)` walks with `jwalk` on its own thread. **Every IO
  error (permission denied, vanished file) is sent as `ScanMessage::Error` and skipped — never
  unwrap/panic on filesystem calls.** Calls `ctx.request_repaint()` so the UI updates live.
- `src/analyzer.rs` — operates on **indices into the master `Vec<FileMetadata>`, never mutating or
  cloning it.** `apply()` returns matching indices; `sort()` reorders them in place (both use rayon's
  `par_sort_unstable_by`).
- `src/ui/` — panels are **free functions that borrow only the fields they need** (not `&mut App`),
  to avoid borrow-checker conflicts across simultaneously-open panels. `mod.rs` holds shared
  formatters (`human_bytes`, `format_time`). Panels signal changes via return values
  (`filter_bar` → `bool` changed; `file_table` → `Option<SortKey>` clicked) and the caller decides
  whether to `rebuild_view`.
- `src/main.rs` — `App` holds `files` (master list) + `view` (filtered/sorted index list that the
  table renders). `rebuild_view()` is the single recompute path (apply filter, then sort); call it
  after any filter or sort change.

## Conventions / invariants

- **`files` is the source of truth; `view` is a derived `Vec<usize>` into it.** The table renders
  `files[view[i]]`. Keep these in sync via `rebuild_view()`.
- **Virtualized rendering:** `file_table` uses `egui_extras` `TableBuilder::body.rows(...)`, which
  only builds visible rows — required so the table scales to millions of files. Don't replace it with
  an eager loop over all rows.
- Dependency versions are pinned to the `0.x` lines compatible with the toolchain (egui/eframe/
  egui_extras `0.29`, sysinfo `0.32`, rfd `0.15`). egui has frequent breaking API changes between
  minor versions — bump these together and re-check the panel code.
- `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` in `main.rs` hides the console
  in release but keeps it in debug for logging.
