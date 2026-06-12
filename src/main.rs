//! OxideDisk Analyzer — a single-binary Windows disk analyzer.
//!
//! Vertical slice: pick a folder/drive, scan it (multi-threaded, crash-proof on
//! permission errors), and explore the results in a virtualized, sortable and
//! filterable table with a disk-capacity dashboard.

// Hide the console window on Windows release builds (keep it for `cargo run`/debug).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analyzer;
mod models;
mod scanner;
mod ui;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use models::{DiskInfo, FileMetadata, FilterState, ScanMessage, SortKey};

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("OxideDisk Analyzer"),
        ..Default::default()
    };

    eframe::run_native(
        "OxideDisk Analyzer",
        native_options,
        Box::new(|_cc| Ok(Box::<App>::default())),
    )
}

struct App {
    files: Vec<FileMetadata>,
    /// Indices into `files` after filtering + sorting — what the table renders.
    view: Vec<usize>,
    filter: FilterState,
    sort: (SortKey, bool),

    rx: Option<Receiver<ScanMessage>>,
    scanning: bool,
    /// (files scanned, bytes scanned)
    progress: (usize, u64),
    errors: Vec<String>,
    disk: Option<DiskInfo>,
    show_log: bool,
}

impl Default for App {
    fn default() -> Self {
        App {
            files: Vec::new(),
            view: Vec::new(),
            filter: FilterState::default(),
            sort: (SortKey::Size, false), // largest first by default
            rx: None,
            scanning: false,
            progress: (0, 0),
            errors: Vec::new(),
            disk: None,
            show_log: false,
        }
    }
}

impl App {
    /// Recompute the visible index list from the current filter + sort.
    fn rebuild_view(&mut self) {
        self.view = analyzer::apply(&self.files, &self.filter);
        let (key, asc) = self.sort;
        analyzer::sort(&mut self.view, &self.files, key, asc);
    }

    /// Start a fresh scan rooted at `root`.
    fn start_scan(&mut self, root: PathBuf, ctx: &egui::Context) {
        self.files.clear();
        self.view.clear();
        self.errors.clear();
        self.progress = (0, 0);
        self.scanning = true;
        self.disk = disk_for(&root);

        let (tx, rx) = channel();
        self.rx = Some(rx);
        scanner::spawn_scan(root, tx, ctx.clone());
    }

    /// Drain any pending scan messages.
    fn pump_scan(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.rx {
            for msg in rx.try_iter() {
                match msg {
                    ScanMessage::Progress { scanned, bytes } => {
                        self.progress = (scanned, bytes);
                    }
                    ScanMessage::Error(e) => {
                        // Cap the log so a pathological scan can't grow unbounded.
                        if self.errors.len() < 10_000 {
                            self.errors.push(e);
                        }
                    }
                    ScanMessage::Done(files) => {
                        self.progress.0 = files.len();
                        self.progress.1 = files.iter().map(|f| f.size).sum();
                        self.files = files;
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.scanning = false;
            self.rx = None;
            self.rebuild_view();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_scan();

        egui::TopBottomPanel::top("dashboard").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.scanning, egui::Button::new("📁 Select folder"))
                    .clicked()
                {
                    if let Some(root) = rfd::FileDialog::new().pick_folder() {
                        self.start_scan(root, ctx);
                    }
                }
                ui.checkbox(&mut self.show_log, format!("Log ({})", self.errors.len()));
            });
            ui.add_space(4.0);
            ui::dashboard::show(ui, &self.disk, self.scanning, self.progress, self.view.len());
        });

        egui::SidePanel::left("filters")
            .resizable(true)
            .default_width(200.0)
            .show(ctx, |ui| {
                if ui::filter_bar::show(ui, &mut self.filter) && !self.files.is_empty() {
                    self.rebuild_view();
                }
            });

        if self.show_log {
            egui::TopBottomPanel::bottom("log")
                .resizable(true)
                .default_height(140.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("Scan log — {} issues", self.errors.len()));
                        if ui.button("Clear").clicked() {
                            self.errors.clear();
                        }
                    });
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for e in &self.errors {
                                ui.label(e);
                            }
                        });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.files.is_empty() && !self.scanning {
                ui.centered_and_justified(|ui| {
                    ui.label("No data yet — click “Select folder” to scan a drive or directory.");
                });
                return;
            }
            if let Some(key) = ui::file_table::show(ui, &self.files, &self.view, self.sort) {
                // Toggle direction if the same column was clicked again.
                let (cur_key, cur_asc) = self.sort;
                self.sort = if cur_key == key {
                    (key, !cur_asc)
                } else {
                    (key, true)
                };
                self.rebuild_view();
            }
        });
    }
}

/// Find the disk whose mount point contains `root` (matched by path prefix).
fn disk_for(root: &Path) -> Option<DiskInfo> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best: Option<DiskInfo> = None;
    let mut best_len = 0usize;
    for disk in disks.list() {
        let mount = disk.mount_point();
        if root.starts_with(mount) {
            let len = mount.as_os_str().len();
            if len >= best_len {
                best_len = len;
                best = Some(DiskInfo {
                    mount: mount.to_string_lossy().into_owned(),
                    total: disk.total_space(),
                    available: disk.available_space(),
                });
            }
        }
    }
    best
}
