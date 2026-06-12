//! Core data structures shared across the scanner, analyzer, and UI.

use std::path::PathBuf;
use std::time::SystemTime;

/// Metadata captured for a single file during a scan.
#[derive(Clone, Debug)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: u64,
    /// Lowercased extension without the dot (empty if none).
    pub extension: String,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
}

impl FileMetadata {
    /// The file name (last path component) as a display string.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.path.to_string_lossy().into_owned())
    }

    pub fn category(&self) -> Category {
        Category::of(&self.extension)
    }
}

/// Capacity info for the disk being analyzed.
#[derive(Clone, Debug)]
pub struct DiskInfo {
    pub mount: String,
    pub total: u64,
    pub available: u64,
}

impl DiskInfo {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    /// Fraction used in `0.0..=1.0` (0 when total is unknown).
    pub fn used_fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.used() as f64 / self.total as f64) as f32
        }
    }
}

/// Coarse file categories used by the quick filters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Images,
    Video,
    Audio,
    Documents,
    Archives,
    System,
    Other,
}

impl Category {
    pub fn of(ext: &str) -> Category {
        match ext {
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "tiff" | "heic" | "svg" => {
                Category::Images
            }
            "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" => Category::Video,
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" => Category::Audio,
            "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "txt" | "md" | "csv"
            | "rtf" | "odt" => Category::Documents,
            "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "iso" => Category::Archives,
            "sys" | "dll" | "exe" | "msi" | "cab" | "drv" | "tmp" | "log" => Category::System,
            _ => Category::Other,
        }
    }

    pub const ALL: [Category; 7] = [
        Category::Images,
        Category::Video,
        Category::Audio,
        Category::Documents,
        Category::Archives,
        Category::System,
        Category::Other,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Category::Images => "Images",
            Category::Video => "Video",
            Category::Audio => "Audio",
            Category::Documents => "Documents",
            Category::Archives => "Archives",
            Category::System => "System",
            Category::Other => "Other",
        }
    }
}

/// Column the file table is sorted by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Modified,
}

/// Active filtering state, mutated by the side panel.
#[derive(Clone, Debug)]
pub struct FilterState {
    /// Minimum size in bytes (0 = no minimum).
    pub min_size: u64,
    /// `None` = all categories.
    pub category: Option<Category>,
    /// Keep only files older (last modified) than this many days. `None` = no age filter.
    pub min_age_days: Option<u64>,
    /// When set, keep only the N largest files after other filters.
    pub top_n: Option<usize>,
}

impl Default for FilterState {
    fn default() -> Self {
        FilterState {
            min_size: 0,
            category: None,
            min_age_days: None,
            top_n: None,
        }
    }
}

/// Messages streamed from the background scan thread to the UI.
#[derive(Debug)]
pub enum ScanMessage {
    Progress { scanned: usize, bytes: u64 },
    Error(String),
    Done(Vec<FileMetadata>),
}
