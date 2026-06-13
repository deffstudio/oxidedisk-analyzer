//! OxideDisk Analyzer — a single-binary Windows disk analyzer.
//!
//! Vertical slice: pick a folder/drive, scan it (multi-threaded, crash-proof on
//! permission errors), and explore the results in a virtualized, sortable and
//! filterable table with a disk-capacity dashboard.

// Hide the console window on Windows release builds (keep it for `cargo run`/debug).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analyzer;
mod cleanup;
mod duplicates;
mod elevation;
mod models;
mod scanner;
mod ui;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use models::{
    DiskInfo, DupMessage, DuplicateGroup, FileMetadata, FilterState, ScanMessage, Settings, SortKey,
};

/// Which view the central panel shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Files,
    Duplicates,
    Cleanup,
}

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
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

struct App {
    /// Master file list, shared with background threads via `Arc` (never mutated
    /// after a scan completes).
    files: Arc<Vec<FileMetadata>>,
    /// Indices into `files` after filtering + sorting — what the table renders.
    view: Vec<usize>,
    filter: FilterState,
    sort: (SortKey, bool),
    view_mode: ViewMode,

    rx: Option<Receiver<ScanMessage>>,
    scanning: bool,
    /// (files scanned, bytes scanned)
    progress: (usize, u64),
    errors: Vec<String>,
    disk: Option<DiskInfo>,
    show_log: bool,

    dup_rx: Option<Receiver<DupMessage>>,
    finding_dups: bool,
    /// (files hashed, total candidates) for the current find pass.
    dup_progress: (usize, usize),
    dup_groups: Vec<DuplicateGroup>,

    /// Whether the cleanup dry-run confirmation dialog is open.
    confirm_cleanup: bool,
    /// Last directory scanned — persisted, and used as the picker's start dir.
    last_root: Option<PathBuf>,
    /// Whether this process is running with administrator rights.
    elevated: bool,
    /// Set when launched elevated with `--cleanup`: auto-open temp cleanup on
    /// the first frame.
    pending_cleanup: bool,
}

impl Default for App {
    fn default() -> Self {
        App {
            files: Arc::new(Vec::new()),
            view: Vec::new(),
            filter: FilterState::default(),
            sort: (SortKey::Size, false), // largest first by default
            view_mode: ViewMode::Files,
            rx: None,
            scanning: false,
            progress: (0, 0),
            errors: Vec::new(),
            disk: None,
            show_log: false,
            dup_rx: None,
            finding_dups: false,
            dup_progress: (0, 0),
            dup_groups: Vec::new(),
            confirm_cleanup: false,
            last_root: None,
            elevated: elevation::is_elevated(),
            pending_cleanup: std::env::args().any(|a| a == elevation::CLEANUP_FLAG),
        }
    }
}

impl App {
    /// Build the app, restoring persisted settings from eframe storage.
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = App::default();
        if let Some(storage) = cc.storage {
            if let Some(s) = eframe::get_value::<Settings>(storage, eframe::APP_KEY) {
                app.filter = s.filter;
                if let Some(key) = s.sort_key {
                    app.sort = (key, s.sort_asc);
                }
                app.show_log = s.show_log;
                app.last_root = s.last_root;
            }
        }
        app
    }

    /// Snapshot the current preferences for persistence.
    fn settings(&self) -> Settings {
        Settings {
            last_root: self.last_root.clone(),
            sort_key: Some(self.sort.0),
            sort_asc: self.sort.1,
            filter: self.filter.clone(),
            show_log: self.show_log,
        }
    }

    /// Recompute the visible index list from the current filter + sort.
    fn rebuild_view(&mut self) {
        self.view = analyzer::apply(&self.files, &self.filter);
        let (key, asc) = self.sort;
        analyzer::sort(&mut self.view, &self.files, key, asc);
    }

    /// Start a fresh scan rooted at `roots`.
    fn start_scan(&mut self, roots: Vec<PathBuf>, ctx: &egui::Context) {
        self.files = Arc::new(Vec::new());
        self.view.clear();
        self.errors.clear();
        self.dup_groups.clear();
        self.dup_progress = (0, 0);
        self.confirm_cleanup = false;
        self.view_mode = ViewMode::Files;
        self.progress = (0, 0);
        self.scanning = true;
        
        // Use the first root for disk info if available.
        if let Some(root) = roots.first() {
            self.disk = disk_for(root);
        } else {
            self.disk = None;
        }

        let (tx, rx) = channel();
        self.rx = Some(rx);
        scanner::spawn_scan(roots, tx, ctx.clone());
    }

    /// Targeted scan of common temp folders.
    fn start_temp_cleanup_scan(&mut self, ctx: &egui::Context) {
        let roots: Vec<PathBuf> = cleanup::get_known_temp_folders()
            .into_iter()
            .map(|f| f.path)
            .collect();
        
        if roots.is_empty() {
            self.errors.push("No common temp folders found.".to_string());
            return;
        }

        self.start_scan(roots, ctx);
        self.view_mode = ViewMode::Cleanup;
    }

    /// Kick off duplicate detection over the current file list.
    fn start_find_dups(&mut self, ctx: &egui::Context) {
        self.dup_groups.clear();
        self.dup_progress = (0, 0);
        self.finding_dups = true;

        let (tx, rx) = channel();
        self.dup_rx = Some(rx);
        duplicates::spawn_find(Arc::clone(&self.files), tx, ctx.clone());
    }

    /// Drain any pending duplicate-finder messages.
    fn pump_dups(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.dup_rx {
            for msg in rx.try_iter() {
                match msg {
                    DupMessage::Progress { hashed, total } => {
                        self.dup_progress = (hashed, total);
                    }
                    DupMessage::Error(e) => {
                        if self.errors.len() < 10_000 {
                            self.errors.push(e);
                        }
                    }
                    DupMessage::Done(groups) => {
                        self.dup_groups = groups;
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.finding_dups = false;
            self.dup_rx = None;
            self.view_mode = ViewMode::Duplicates;
        }
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
                        self.files = Arc::new(files);
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
    /// Persist user preferences (eframe also persists window geometry).
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings());
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump_scan();
        self.pump_dups();

        // Launched elevated with `--cleanup`: jump straight to the temp scan.
        if self.pending_cleanup {
            self.pending_cleanup = false;
            self.start_temp_cleanup_scan(ctx);
        }

        egui::TopBottomPanel::top("dashboard").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.scanning, egui::Button::new("📁 Select folder"))
                    .clicked()
                {
                    let mut dialog = rfd::FileDialog::new();
                    if let Some(root) = &self.last_root {
                        dialog = dialog.set_directory(root);
                    }
                    if let Some(root) = dialog.pick_folder() {
                        self.last_root = Some(root.clone());
                        self.start_scan(vec![root], ctx);
                    }
                }

                if ui
                    .add_enabled(!self.scanning, egui::Button::new("🧹 Temp Cleanup"))
                    .clicked()
                {
                    self.start_temp_cleanup_scan(ctx);
                }

                let can_find = !self.files.is_empty() && !self.scanning && !self.finding_dups;
                if ui
                    .add_enabled(can_find, egui::Button::new("🔍 Find Duplicates"))
                    .clicked()
                {
                    self.start_find_dups(ctx);
                }

                if self.finding_dups {
                    ui.spinner();
                    ui.label(format!(
                        "Hashing {}/{}",
                        self.dup_progress.0, self.dup_progress.1
                    ));
                } else if !self.files.is_empty() {
                    ui.selectable_value(&mut self.view_mode, ViewMode::Files, "Files");
                    
                    if !self.dup_groups.is_empty() {
                        ui.selectable_value(
                            &mut self.view_mode,
                            ViewMode::Duplicates,
                            format!("Duplicates ({})", self.dup_groups.len()),
                        );
                    }
                    
                    // Always show Cleanup tab if we are in it, or if it's relevant.
                    // For now, let's just show it if we are in it.
                    if self.view_mode == ViewMode::Cleanup {
                        ui.selectable_value(&mut self.view_mode, ViewMode::Cleanup, "Cleanup");
                    }
                }

                ui.checkbox(&mut self.show_log, format!("Log ({})", self.errors.len()));
            });
            ui.add_space(4.0);
            ui::dashboard::show(
                ui,
                &self.disk,
                self.scanning,
                self.progress,
                self.view.len(),
                self.elevated,
            );
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
            match self.view_mode {
                ViewMode::Cleanup => {
                    match ui::cleanup::show(
                        ui,
                        &self.files,
                        &self.view,
                        self.elevated,
                        &mut self.confirm_cleanup,
                        |path| cleanup::delete_to_trash(path),
                    ) {
                        ui::cleanup::CleanupAction::Cleaned => {
                            self.files = Arc::new(Vec::new());
                            self.view.clear();
                        }
                        ui::cleanup::CleanupAction::Elevate => {
                            match elevation::relaunch_as_admin(&[elevation::CLEANUP_FLAG]) {
                                // Elevated instance launched — hand off and quit.
                                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                                Err(e) => self.errors.push(e),
                            }
                        }
                        ui::cleanup::CleanupAction::None => {}
                    }
                }
                ViewMode::Duplicates => {
                    ui::duplicates::show(ui, &self.files, &self.dup_groups);
                }
                ViewMode::Files => {
                    if let Some(key) = ui::file_table::show(ui, &self.files, &self.view, self.sort)
                    {
                        // Toggle direction if the same column was clicked again.
                        let (cur_key, cur_asc) = self.sort;
                        self.sort = if cur_key == key {
                            (key, !cur_asc)
                        } else {
                            (key, true)
                        };
                        self.rebuild_view();
                    }
                }
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
