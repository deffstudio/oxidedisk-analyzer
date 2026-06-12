//! Left side panel with quick filters. Mutates `FilterState`; returns `true`
//! when anything changed so the caller can recompute the view.

use crate::models::{Category, FilterState};
use crate::ui::human_bytes;

const MB: u64 = 1024 * 1024;

pub fn show(ui: &mut egui::Ui, filter: &mut FilterState) -> bool {
    let mut changed = false;

    ui.heading("Filters");
    ui.add_space(6.0);

    ui.label("Quick filters");
    if ui.button("Large Files (>500 MB)").clicked() {
        *filter = FilterState {
            min_size: 500 * MB,
            ..FilterState::default()
        };
        changed = true;
    }
    if ui.button("Top 100 Largest").clicked() {
        *filter = FilterState {
            top_n: Some(100),
            ..FilterState::default()
        };
        changed = true;
    }
    if ui.button("Stale (>90 days)").clicked() {
        *filter = FilterState {
            min_age_days: Some(90),
            ..FilterState::default()
        };
        changed = true;
    }
    if ui.button("Reset").clicked() {
        *filter = FilterState::default();
        changed = true;
    }

    ui.separator();

    // Minimum size slider, expressed in MB.
    let mut min_mb = (filter.min_size / MB) as f64;
    ui.label(format!("Min size: {}", human_bytes(filter.min_size)));
    if ui
        .add(egui::Slider::new(&mut min_mb, 0.0..=5000.0).suffix(" MB"))
        .changed()
    {
        filter.min_size = (min_mb as u64) * MB;
        changed = true;
    }

    ui.separator();

    // Category selector.
    ui.label("Category");
    let current = filter.category.map(|c| c.label()).unwrap_or("All");
    egui::ComboBox::from_id_salt("category_combo")
        .selected_text(current)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(filter.category.is_none(), "All")
                .clicked()
            {
                filter.category = None;
                changed = true;
            }
            for cat in Category::ALL {
                if ui
                    .selectable_label(filter.category == Some(cat), cat.label())
                    .clicked()
                {
                    filter.category = Some(cat);
                    changed = true;
                }
            }
        });

    ui.separator();

    // Age filter toggle.
    let mut age_on = filter.min_age_days.is_some();
    if ui.checkbox(&mut age_on, "Older than (days)").changed() {
        filter.min_age_days = if age_on { Some(90) } else { None };
        changed = true;
    }
    if let Some(days) = filter.min_age_days.as_mut() {
        let mut d = *days as f64;
        if ui
            .add(egui::Slider::new(&mut d, 1.0..=3650.0).suffix(" d"))
            .changed()
        {
            *days = d as u64;
            changed = true;
        }
    }

    changed
}
