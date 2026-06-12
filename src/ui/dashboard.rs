//! Top dashboard: disk capacity bar + scan status.

use crate::models::DiskInfo;
use crate::ui::human_bytes;

/// Draw the dashboard into the given `ui`.
pub fn show(
    ui: &mut egui::Ui,
    disk: &Option<DiskInfo>,
    scanning: bool,
    progress: (usize, u64),
    shown: usize,
) {
    ui.horizontal(|ui| {
        ui.heading("OxideDisk Analyzer");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if scanning {
                ui.spinner();
                ui.label(format!(
                    "Scanning… {} files, {}",
                    progress.0,
                    human_bytes(progress.1)
                ));
            } else if progress.0 > 0 {
                ui.label(format!(
                    "{} files indexed · {} shown",
                    progress.0, shown
                ));
            }
        });
    });

    ui.add_space(4.0);

    match disk {
        Some(d) => {
            let frac = d.used_fraction();
            ui.label(format!(
                "Drive {} — {} used of {} ({} free)",
                d.mount,
                human_bytes(d.used()),
                human_bytes(d.total),
                human_bytes(d.available),
            ));
            ui.add(
                egui::ProgressBar::new(frac)
                    .text(format!("{:.0}% used", frac * 100.0))
                    .desired_width(f32::INFINITY),
            );
        }
        None => {
            ui.label("Select a folder to begin scanning.");
        }
    }
}
