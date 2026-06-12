//! egui panels and shared formatting helpers.

pub mod dashboard;
pub mod file_table;
pub mod filter_bar;

use std::time::SystemTime;

use chrono::{DateTime, Local};

/// Format a byte count as a human-readable string (e.g. `1.4 GB`).
pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}

/// Format a `SystemTime` as a local `YYYY-MM-DD HH:MM` string.
pub fn format_time(time: Option<SystemTime>) -> String {
    match time {
        Some(t) => {
            let dt: DateTime<Local> = t.into();
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
        None => "—".to_string(),
    }
}
