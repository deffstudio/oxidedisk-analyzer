//! Duplicate-sets view: a summary line plus one collapsible group per set.

use crate::models::{DuplicateGroup, FileMetadata};
use crate::ui::human_bytes;

pub fn show(ui: &mut egui::Ui, files: &[FileMetadata], groups: &[DuplicateGroup]) {
    if groups.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label("No duplicates found. Run “Find Duplicates” after a scan.");
        });
        return;
    }

    let reclaimable: u64 = groups.iter().map(|g| g.wasted()).sum();
    let dup_files: usize = groups.iter().map(|g| g.members.len()).sum();
    ui.strong(format!(
        "{} duplicate sets · {} files · {} reclaimable",
        groups.len(),
        dup_files,
        human_bytes(reclaimable),
    ));
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (gi, g) in groups.iter().enumerate() {
                let header = format!(
                    "{} copies · {} each · {} wasted",
                    g.members.len(),
                    human_bytes(g.size),
                    human_bytes(g.wasted()),
                );
                egui::CollapsingHeader::new(header)
                    .id_salt(gi)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&g.hash[..g.hash.len().min(16)]).weak());
                        for &m in &g.members {
                            let p = files[m].path.to_string_lossy();
                            ui.label(p.as_ref()).on_hover_text(p.as_ref());
                        }
                    });
            }
        });
}
