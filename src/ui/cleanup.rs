//! UI for cleaning up temporary files.

use std::path::Path;
use crate::cleanup;
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
    confirming: &mut bool,
    delete_fn: impl Fn(&Path) -> Result<(), String>,
) -> CleanupAction {
    let mut action = CleanupAction::None;

    // Partition the scan into what we'll recycle vs. protected critical paths
    // we refuse to touch (defense-in-depth — temp scans shouldn't surface these,
    // but never delete inside System32/Program Files/etc. if they do).
    let protected_roots = cleanup::protected_roots();
    let mut deletable: Vec<usize> = Vec::with_capacity(view.len());
    let mut protected: Vec<usize> = Vec::new();
    for &i in view {
        if cleanup::is_protected(&files[i].path, &protected_roots) {
            protected.push(i);
        } else {
            deletable.push(i);
        }
    }
    let deletable_size: u64 = deletable.iter().map(|&i| files[i].size).sum();
    let total_size: u64 = view.iter().map(|&i| files[i].size).sum();

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.strong(format!(
                "Found {} temporary files ({})",
                view.len(),
                human_bytes(total_size)
            ));
            if !protected.is_empty() {
                ui.weak(format!("· {} protected (skipped)", protected.len()));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !deletable.is_empty(),
                        egui::Button::new("🗑 Clean All (Move to Recycle Bin)"),
                    )
                    .clicked()
                {
                    *confirming = true;
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
                        let is_protected = protected.contains(&i);
                        ui.horizontal(|ui| {
                            ui.label(format!("[{}]", human_bytes(f.size)));
                            if is_protected {
                                ui.label("🔒")
                                    .on_hover_text("Protected system path — will not be touched.");
                                ui.weak(f.path.to_string_lossy().into_owned());
                            } else {
                                ui.label(f.path.to_string_lossy().into_owned())
                                    .on_hover_text(f.path.to_string_lossy().into_owned());
                            }
                        });
                    }
                });
        }
    });

    // Dry-run confirmation modal: nothing is deleted until the user confirms here.
    if *confirming {
        if let Some(confirmed) = confirm_modal(
            ui.ctx(),
            deletable.len(),
            deletable_size,
            protected.len(),
        ) {
            if confirmed {
                for &i in &deletable {
                    if let Err(e) = delete_fn(&files[i].path) {
                        eprintln!("{}", e);
                    }
                }
                action = CleanupAction::Cleaned;
            }
            *confirming = false;
        }
    }

    action
}

/// Centered confirmation dialog. Returns `Some(true)` if the user confirmed,
/// `Some(false)` if they cancelled, `None` if the dialog is still open.
fn confirm_modal(
    ctx: &egui::Context,
    count: usize,
    size: u64,
    protected: usize,
) -> Option<bool> {
    // Dim the background so the dialog reads as modal.
    egui::Area::new(egui::Id::new("cleanup_modal_shade"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            let screen = ui.ctx().screen_rect();
            ui.painter()
                .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(128));
        });

    let mut result = None;
    egui::Window::new("Confirm cleanup")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!(
                "Move {} file(s) ({}) to the Recycle Bin?",
                count,
                human_bytes(size)
            ));
            ui.add_space(4.0);
            ui.weak("Items go to the Recycle Bin and can be restored.");
            if protected > 0 {
                ui.weak(format!(
                    "{} protected system file(s) will be skipped.",
                    protected
                ));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Move to Recycle Bin").clicked() {
                    result = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    result = Some(false);
                }
            });
        });

    // Allow Esc to cancel.
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        result = Some(false);
    }
    result
}
