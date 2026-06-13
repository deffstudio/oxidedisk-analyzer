# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

OxideDisk Analyzer — a single-binary Windows 11 disk-usage analyzer written in Rust with an
`egui`/`eframe` GUI. It can scan a folder/drive, show a disk-capacity dashboard, explore files in a
virtualized sortable/filterable table, detect duplicate files (blake3), clean known temp/junk folders
(move to Recycle Bin), and elevate to Administrator on demand for protected system folders. The app
runs unprivileged and only relaunches elevated when the user invokes cleanup that needs it (no
launch-time admin manifest).

## Commands

```bash
cargo run              # build + launch the GUI (console window kept in debug)
cargo build --release  # single optimized exe (LTO on); console hidden via windows_subsystem
cargo check            # fast type-check during iteration
cargo clippy           # lint
cargo fmt              # format
```

`cargo test` runs the suite (currently the blake3 hashing tests in `src/duplicates.rs`); a single
test: `cargo test <name>`.

## Architecture

The app is **single-threaded UI + background worker threads**, each communicating over its own `mpsc`
channel. This is the central design constraint: the egui loop must never block, so all filesystem and
hashing work happens off-thread and results stream back as messages drained per frame.

**Data flow:** `main.rs` (`App`) owns the state and orchestrates everything → `scanner::spawn_scan`
walks the tree on a worker thread, sending `ScanMessage`s → `App::pump_scan` drains them each frame →
on `Done`, `analyzer` filters + sorts → the `ui` panels render. Duplicate detection
(`duplicates::spawn_find`) runs the same pattern on its own thread/channel (`DupMessage` / `pump_dups`).
The master `files` list is held in an `Arc<Vec<FileMetadata>>` so it can be shared cheaply with the
dup-finder thread without cloning.

- `src/models.rs` — shared types: `FileMetadata`, `DiskInfo`, `Category` (extension→category map),
  `SortKey`, `FilterState`, and the `ScanMessage` enum (`Progress` / `Error` / `Done`) that defines
  the thread→UI protocol.
- `src/scanner.rs` — `spawn_scan(root, tx, ctx)` walks with `jwalk` on its own thread. **Every IO
  error (permission denied, vanished file) is sent as `ScanMessage::Error` and skipped — never
  unwrap/panic on filesystem calls.** Calls `ctx.request_repaint()` so the UI updates live.
- `src/analyzer.rs` — operates on **indices into the master `Vec<FileMetadata>`, never mutating or
  cloning it.** `apply()` returns matching indices; `sort()` reorders them in place (both use rayon's
  `par_sort_unstable_by`).
- `src/duplicates.rs` — `spawn_find(files, tx, ctx)` hashes on its own thread via a **3-stage funnel**
  (group by size → blake3 of first 16 KB prefix → full blake3 confirm), processing candidates in
  chunks of 256 with rayon and reporting `DupMessage::Progress` per chunk. **IO errors are sent as
  `DupMessage::Error` and the file is dropped — never panic.** Has unit tests (`cargo test`).
- `src/cleanup.rs` — `get_known_temp_folders()` returns the temp/cache locations that exist on this
  machine; `delete_to_trash()` moves a path to the Recycle Bin via `trash` (recoverable, not a hard
  delete). The targeted temp scan reuses `scanner::spawn_scan` over these roots. `protected_roots()` +
  `is_protected()` form a hardcoded critical-path blacklist (System32/SysWOW64/WinSxS/Program Files)
  the cleanup UI always skips. The **🗑 Clean All** button opens a dry-run confirmation dialog
  (`ui::cleanup::confirm_modal`) — nothing is recycled until the user confirms.
- `src/elevation.rs` — on-demand UAC. `is_elevated()` checks the process token; `relaunch_as_admin()`
  re-launches the same exe with the `runas` verb (`ShellExecuteW`) and the `CLEANUP_FLAG` so the
  elevated instance jumps straight to the cleanup view, then the unprivileged instance closes. Windows
  bits are `#[cfg(windows)]` with a stub fallback. Uses `windows-sys` (a `cfg(windows)` dependency).
- `src/ui/` — panels are **free functions that borrow only the fields they need** (not `&mut App`),
  to avoid borrow-checker conflicts across simultaneously-open panels. `mod.rs` holds shared
  formatters (`human_bytes`, `format_time`). Panels signal changes via return values
  (`filter_bar` → `bool` changed; `file_table` → `Option<SortKey>` clicked) and the caller decides
  whether to `rebuild_view`.
- `src/main.rs` — `App` holds `files` (master list) + `view` (filtered/sorted index list that the
  table renders). `rebuild_view()` is the single recompute path (apply filter, then sort); call it
  after any filter or sort change. `App::new(cc)` restores the persisted `Settings` (filter, sort,
  last root, log toggle) and `App::save` writes them back via eframe storage (`eframe` is built with
  the `persistence` feature; `serde` derives live on the persisted `models` types). Window geometry
  is persisted automatically by eframe.

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
