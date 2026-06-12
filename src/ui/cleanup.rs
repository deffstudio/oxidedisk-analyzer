//! UI for cleaning up temporary files.

use std::path::Path;
use crate::models::FileMetadata;
use crate::ui::human_bytes;

pub fn show(
    ui: &mut egui::Ui,
    files: &[FileMetadata],
    view: &[usize],
    delete_fn: impl Fn(&Path) -> Result<(), String>,
) -> bool {
    let mut cleaned = false;
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
                    cleaned = true;
                }
            });
        });

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

    cleaned
}
