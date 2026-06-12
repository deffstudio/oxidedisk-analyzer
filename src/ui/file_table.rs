//! Central virtualized file table. Only visible rows are rendered, so it scales
//! to millions of files. Returns `Some(SortKey)` when a column header is clicked.

use egui_extras::{Column, TableBuilder};

use crate::models::{FileMetadata, SortKey};
use crate::ui::{format_time, human_bytes};

const ROW_HEIGHT: f32 = 18.0;

pub fn show(
    ui: &mut egui::Ui,
    files: &[FileMetadata],
    view: &[usize],
    sort: (SortKey, bool),
) -> Option<SortKey> {
    let (active_key, ascending) = sort;
    let mut clicked: Option<SortKey> = None;

    let arrow = |key: SortKey| -> &'static str {
        if key == active_key {
            if ascending {
                " ▲"
            } else {
                " ▼"
            }
        } else {
            ""
        }
    };

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::remainder().at_least(160.0).clip(true)) // Name
        .column(Column::exact(90.0)) // Size
        .column(Column::exact(70.0)) // Category
        .column(Column::exact(130.0)) // Modified
        .column(Column::remainder().at_least(200.0).clip(true)) // Path
        .header(22.0, |mut header| {
            header.col(|ui| {
                if ui
                    .button(format!("Name{}", arrow(SortKey::Name)))
                    .clicked()
                {
                    clicked = Some(SortKey::Name);
                }
            });
            header.col(|ui| {
                if ui
                    .button(format!("Size{}", arrow(SortKey::Size)))
                    .clicked()
                {
                    clicked = Some(SortKey::Size);
                }
            });
            header.col(|ui| {
                ui.strong("Category");
            });
            header.col(|ui| {
                if ui
                    .button(format!("Modified{}", arrow(SortKey::Modified)))
                    .clicked()
                {
                    clicked = Some(SortKey::Modified);
                }
            });
            header.col(|ui| {
                ui.strong("Path");
            });
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, view.len(), |mut row| {
                let f = &files[view[row.index()]];
                row.col(|ui| {
                    ui.label(f.name());
                });
                row.col(|ui| {
                    ui.label(human_bytes(f.size));
                });
                row.col(|ui| {
                    ui.label(f.category().label());
                });
                row.col(|ui| {
                    ui.label(format_time(f.modified));
                });
                row.col(|ui| {
                    ui.label(f.path.to_string_lossy().into_owned())
                        .on_hover_text(f.path.to_string_lossy().into_owned());
                });
            });
        });

    clicked
}
