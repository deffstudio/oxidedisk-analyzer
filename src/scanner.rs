//! Background, multi-threaded directory scanning built on `jwalk`.
//!
//! [`spawn_scan`] runs the walk on its own thread and streams [`ScanMessage`]s
//! back to the UI over an mpsc channel, requesting a repaint as results arrive
//! so the window updates live and never blocks.

use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;

use jwalk::WalkDir;

use crate::models::{FileMetadata, ScanMessage};

/// How many files to process between `Progress` updates.
const PROGRESS_EVERY: usize = 2000;

/// Spawn a scan of one or more `roots`, streaming messages over `tx` and repainting `ctx`.
pub fn spawn_scan(roots: Vec<PathBuf>, tx: Sender<ScanMessage>, ctx: egui::Context) {
    thread::spawn(move || {
        let mut files: Vec<FileMetadata> = Vec::new();
        let mut total_bytes: u64 = 0;
        let mut since_update: usize = 0;

        for root in roots {
            for entry in WalkDir::new(&root).skip_hidden(false) {
                let entry = match entry {
                    Ok(e) => e,
                    Err(err) => {
                        // Permission denied, vanished file, etc. — log and keep going.
                        let _ = tx.send(ScanMessage::Error(err.to_string()));
                        continue;
                    }
                };

                // Only files contribute to the table; directories are traversed only.
                if entry.file_type().is_dir() {
                    continue;
                }

                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(err) => {
                        let _ = tx.send(ScanMessage::Error(format!(
                            "{}: {}",
                            entry.path().display(),
                            err
                        )));
                        continue;
                    }
                };

                let path = entry.path();
                let extension = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();

                total_bytes = total_bytes.saturating_add(meta.len());
                files.push(FileMetadata {
                    path,
                    size: meta.len(),
                    extension,
                    modified: meta.modified().ok(),
                    accessed: meta.accessed().ok(),
                });

                since_update += 1;
                if since_update >= PROGRESS_EVERY {
                    since_update = 0;
                    let _ = tx.send(ScanMessage::Progress {
                        scanned: files.len(),
                        bytes: total_bytes,
                    });
                    ctx.request_repaint();
                }
            }
        }

        let _ = tx.send(ScanMessage::Done(files));
        ctx.request_repaint();
    });
}
