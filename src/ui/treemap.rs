//! Treemap panel: a size-proportional, drill-down view of the folder tree.

use egui::{Color32, FontId, Rect, Sense, Stroke, Vec2};

use crate::treemap::{self, Node, Rectf};
use crate::ui::human_bytes;

/// Render the treemap for `root`, navigating into the subtree selected by the
/// index path `zoom` (mutated when the user drills in or up).
pub fn show(ui: &mut egui::Ui, root: &Node, zoom: &mut Vec<usize>) {
    // Resolve the currently focused node by following the zoom path; drop any
    // stale indices (e.g. after a rescan changed the tree).
    let mut node = root;
    let mut depth = 0;
    for &i in zoom.iter() {
        match node.children.get(i) {
            Some(child) => {
                node = child;
                depth += 1;
            }
            None => break,
        }
    }
    zoom.truncate(depth);

    // Breadcrumb + up control.
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!zoom.is_empty(), egui::Button::new("⬆ Up"))
            .clicked()
        {
            zoom.pop();
        }
        ui.separator();
        // Build the crumb labels along the active path.
        let mut crumb = root;
        let mut clicked_level: Option<usize> = None;
        if ui.link(root.name.clone()).clicked() {
            clicked_level = Some(0);
        }
        for (level, &i) in zoom.iter().enumerate() {
            if let Some(child) = crumb.children.get(i) {
                crumb = child;
                ui.label("›");
                if ui.link(child.name.clone()).clicked() {
                    clicked_level = Some(level + 1);
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(format!("{} — {}", node.name, human_bytes(node.size)));
        });
        if let Some(level) = clicked_level {
            zoom.truncate(level);
        }
    });

    ui.separator();

    if node.size == 0 {
        ui.centered_and_justified(|ui| ui.label("This folder is empty."));
        return;
    }

    // Weights: one per child directory, plus a trailing pseudo-tile for bytes in
    // files stored directly in this folder (so the tiles cover the whole area).
    let direct = node.direct_size();
    let mut weights: Vec<f64> = node.children.iter().map(|c| c.size as f64).collect();
    let files_tile = if direct > 0 {
        weights.push(direct as f64);
        Some(weights.len() - 1)
    } else {
        None
    };

    let bounds = ui.available_rect_before_wrap();
    ui.allocate_rect(bounds, Sense::hover());
    if bounds.width() < 4.0 || bounds.height() < 4.0 {
        return;
    }
    let painter = ui.painter_at(bounds);

    let rects = treemap::squarify(
        &weights,
        Rectf::new(
            bounds.min.x as f64,
            bounds.min.y as f64,
            bounds.width() as f64,
            bounds.height() as f64,
        ),
    );

    for (i, r) in rects.iter().enumerate() {
        if r.w <= 0.0 || r.h <= 0.0 {
            continue;
        }
        let rect = Rect::from_min_size(
            egui::pos2(r.x as f32, r.y as f32),
            Vec2::new(r.w as f32, r.h as f32),
        );

        let is_files = files_tile == Some(i);
        let (label, size, drillable, full_path) = if is_files {
            (
                "(files here)".to_string(),
                direct,
                false,
                node.path.to_string_lossy().into_owned(),
            )
        } else {
            let child = &node.children[i];
            (
                child.name.clone(),
                child.size,
                !child.children.is_empty(),
                child.path.to_string_lossy().into_owned(),
            )
        };

        let resp = ui.interact(rect, ui.id().with(("tile", i)), Sense::click());
        let hovered = resp.hovered();

        let fill = tile_color(i, is_files, hovered);
        painter.rect_filled(rect, 2.0, fill);
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_black_alpha(60)));

        // Only label tiles with room to read.
        if rect.width() > 46.0 && rect.height() > 18.0 {
            let pct = size as f64 / node.size as f64 * 100.0;
            let text = format!("{}\n{} · {:.0}%", label, human_bytes(size), pct);
            painter.text(
                rect.min + Vec2::new(4.0, 3.0),
                egui::Align2::LEFT_TOP,
                truncate_to(&text, rect.width()),
                FontId::proportional(12.0),
                Color32::from_gray(20),
            );
        }

        if hovered {
            let pct = size as f64 / node.size as f64 * 100.0;
            let hint = if drillable { "\n(click to open)" } else { "" };
            resp.clone().on_hover_text(format!(
                "{}\n{} ({:.1}% of {}){}",
                full_path,
                human_bytes(size),
                pct,
                node.name,
                hint
            ));
        }
        if drillable && resp.clicked() {
            zoom.push(i);
        }
    }
}

/// A stable, pleasant fill color per tile index. Directly-held files get a muted
/// gray so they read as "not a folder."
fn tile_color(index: usize, is_files: bool, hovered: bool) -> Color32 {
    if is_files {
        return if hovered {
            Color32::from_gray(205)
        } else {
            Color32::from_gray(185)
        };
    }
    // Spread hues around the wheel; golden-angle stepping avoids adjacent clashes.
    let hue = (index as f32 * 0.61803398875).fract();
    let sat = 0.45;
    let val = if hovered { 0.98 } else { 0.86 };
    let rgb = egui::ecolor::Hsva::new(hue, sat, val, 1.0).to_srgba_unmultiplied();
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Rough character truncation so labels don't overflow narrow tiles. Operates on
/// the first line's width budget; multi-line text keeps its newline.
fn truncate_to(text: &str, width: f32) -> String {
    let max_chars = (width / 7.0).floor() as usize; // ~7px per proportional char
    let mut out = String::new();
    for (li, line) in text.lines().enumerate() {
        if li > 0 {
            out.push('\n');
        }
        if line.chars().count() > max_chars && max_chars > 1 {
            out.extend(line.chars().take(max_chars.saturating_sub(1)));
            out.push('…');
        } else {
            out.push_str(line);
        }
    }
    out
}
