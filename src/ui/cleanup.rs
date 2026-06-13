//! UI for cleaning up temporary files.

use std::path::Path;
use crate::models::FileMetadata;
use crate::ui::human_bytes;

/// What the cleanup panel is asking the caller to do this frame.
#[derive(Default)]
pub enum CleanupAction {
    /// Nothing requested.
    #[default]
    None,
    /// Files were moved to the Recycle Bin; the caller should clear the scan.
    Cleaned,
    /// The user asked to relaunch elevated to reach protected folders.
    Elevate,
}

pub fn show(
    ui: &mut egui::Ui,
    files: &[FileMetadata],
    view: &[usize],
    elevated: bool,
    delete_fn: impl Fn(&Path) -> Result<(), String>,
) -> CleanupAction {
    let mut action = CleanupAction::None;
    let total_size: u64 = view.iter().map(|&i| files[i].size).sum();

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.strong(format!(
                "Found {} temporary files ({})",
                view.len(),
                human_bytes(total_size)
            ));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !view.is_empty() && ui.button("🗑 Clean All (Move to Recycle Bin)").clicked() {
                    for &i in view {
                        if let Err(e) = delete_fn(&files[i].path) {
                            eprintln!("{}", e);
                        }
                    }
                    action = CleanupAction::Cleaned;
                }
            });
        });

        // System temp folders (Windows\Temp, Update cache, Prefetch) need admin
        // rights. Offer on-demand elevation rather than failing silently.
        if !elevated {
            ui.horizontal(|ui| {
                if ui
                    .button("🛡 Run as Administrator")
                    .on_hover_text(
                        "Some system folders (Windows\\Temp, Update cache, Prefetch) \
                         require administrator rights. This relaunches OxideDisk elevated.",
                    )
                    .clicked()
                {
                    action = CleanupAction::Elevate;
                }
                ui.weak("— needed to clean protected system folders.");
            });
        }

        ui.separator();

        if view.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No temporary files found in known locations.");
            });
        } else {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for &i in view {
                        let f = &files[i];
                        ui.horizontal(|ui| {
                            ui.label(format!("[{}]", human_bytes(f.size)));
                            ui.label(f.path.to_string_lossy().into_owned())
                                .on_hover_text(f.path.to_string_lossy().into_owned());
                        });
                    }
                });
        }
    });

    action
}
