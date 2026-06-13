//! Folder size aggregation and squarified-treemap layout.
//!
//! The scanner produces a flat `Vec<FileMetadata>`. To draw a size breakdown we
//! aggregate those files into a directory tree (`build`), then lay a node's
//! direct children out as rectangles whose areas are proportional to size using
//! the squarified treemap algorithm (Bruls, Huizing & van Wijk).
//!
//! Layout math runs on plain `f64` rectangles so it's testable without egui.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::FileMetadata;

/// A directory in the aggregated size tree. `size` is the total of all files at
/// or below this directory; `children` are its immediate subdirectories, sorted
/// largest-first. Files held directly in this directory are not individual nodes
/// — their bytes show up as `size` minus the children's combined size.
pub struct Node {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub children: Vec<Node>,
}

impl Node {
    /// Bytes stored in files directly under this directory (not in subdirs).
    pub fn direct_size(&self) -> u64 {
        let child_total: u64 = self.children.iter().map(|c| c.size).sum();
        self.size.saturating_sub(child_total)
    }
}

/// Build the directory tree rooted at the common ancestor of every scanned file.
/// Returns `None` for an empty file list.
pub fn build(files: &[FileMetadata]) -> Option<Node> {
    let root = common_root(files)?;

    // Aggregate sizes per directory and record parent→child directory links.
    let mut sizes: HashMap<PathBuf, u64> = HashMap::new();
    let mut children: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut seen_child: HashMap<PathBuf, std::collections::HashSet<PathBuf>> = HashMap::new();

    for f in files {
        let Some(parent) = f.path.parent() else {
            continue;
        };
        // Walk from the file's parent up to (and including) the root, crediting
        // each directory with this file's size and linking each adjacent pair.
        let mut dir = parent.to_path_buf();
        loop {
            *sizes.entry(dir.clone()).or_insert(0) += f.size;
            if dir == root {
                break;
            }
            match dir.parent() {
                Some(up) => {
                    let up = up.to_path_buf();
                    if seen_child.entry(up.clone()).or_default().insert(dir.clone()) {
                        children.entry(up.clone()).or_default().push(dir.clone());
                    }
                    dir = up;
                }
                None => break,
            }
        }
    }

    Some(build_node(&root, &sizes, &children))
}

/// Recursively assemble a `Node` from the aggregated maps.
fn build_node(
    path: &Path,
    sizes: &HashMap<PathBuf, u64>,
    children: &HashMap<PathBuf, Vec<PathBuf>>,
) -> Node {
    let size = sizes.get(path).copied().unwrap_or(0);
    let mut kids: Vec<Node> = children
        .get(path)
        .map(|cs| cs.iter().map(|c| build_node(c, sizes, children)).collect())
        .unwrap_or_default();
    kids.sort_unstable_by(|a, b| b.size.cmp(&a.size));

    Node {
        name: display_name(path),
        path: path.to_path_buf(),
        size,
        children: kids,
    }
}

/// Last path component for display, falling back to the whole path (e.g. `C:\`).
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// The longest directory prefix shared by every file's parent directory.
fn common_root(files: &[FileMetadata]) -> Option<PathBuf> {
    let mut iter = files.iter().filter_map(|f| f.path.parent());
    let mut root: PathBuf = iter.next()?.to_path_buf();
    for parent in iter {
        while !parent.starts_with(&root) {
            if !root.pop() {
                break;
            }
        }
    }
    Some(root)
}

// ----------------------------------------------------------------------------
// Squarified layout
// ----------------------------------------------------------------------------

/// A plain rectangle for layout math (origin top-left, like egui screen space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rectf {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rectf {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Rectf { x, y, w, h }
    }
    fn area(&self) -> f64 {
        self.w * self.h
    }
    fn shorter_side(&self) -> f64 {
        self.w.min(self.h)
    }
}

/// Squarified treemap: place `weights` (in any positive units) into `bounds`,
/// returning one rect per weight in the same order. Areas are proportional to
/// weights; the algorithm keeps tiles close to square. Non-positive weights get
/// a zero-area rect.
pub fn squarify(weights: &[f64], bounds: Rectf) -> Vec<Rectf> {
    let n = weights.len();
    let mut out = vec![Rectf::new(bounds.x, bounds.y, 0.0, 0.0); n];
    let total: f64 = weights.iter().filter(|w| **w > 0.0).sum();
    if total <= 0.0 || bounds.area() <= 0.0 {
        return out;
    }

    // Work on indices sorted largest-first; scale weights to pixel areas.
    let scale = bounds.area() / total;
    let mut order: Vec<usize> = (0..n).filter(|&i| weights[i] > 0.0).collect();
    order.sort_unstable_by(|&a, &b| weights[b].partial_cmp(&weights[a]).unwrap());
    let areas: Vec<f64> = (0..n).map(|i| weights[i] * scale).collect();

    let mut free = bounds;
    let mut row: Vec<usize> = Vec::new();
    let mut idx = 0;

    while idx < order.len() {
        let i = order[idx];
        let side = free.shorter_side();

        // Try adding the next tile to the current row; keep it if the worst
        // aspect ratio doesn't get worse, otherwise lay the row and start fresh.
        if row.is_empty() || worst(&row, &areas, side) >= worst_with(&row, i, &areas, side) {
            row.push(i);
            idx += 1;
        } else {
            free = place_row(&row, &areas, free, &mut out);
            row.clear();
        }
    }
    if !row.is_empty() {
        place_row(&row, &areas, free, &mut out);
    }
    out
}

/// Worst (largest) aspect ratio in `row` if laid along the side of length `side`.
fn worst(row: &[usize], areas: &[f64], side: f64) -> f64 {
    aspect(row.iter().map(|&i| areas[i]), side)
}

fn worst_with(row: &[usize], extra: usize, areas: &[f64], side: f64) -> f64 {
    aspect(row.iter().chain(std::iter::once(&extra)).map(|&i| areas[i]), side)
}

fn aspect(items: impl Iterator<Item = f64>, side: f64) -> f64 {
    let mut sum = 0.0;
    let mut max = f64::MIN;
    let mut min = f64::MAX;
    for a in items {
        sum += a;
        max = max.max(a);
        min = min.min(a);
    }
    if sum <= 0.0 {
        return f64::MAX;
    }
    let s2 = side * side;
    let sum2 = sum * sum;
    (s2 * max / sum2).max(sum2 / (s2 * min))
}

/// Lay `row` along the shorter side of `free`, writing rects into `out`, and
/// return the remaining free rectangle.
fn place_row(row: &[usize], areas: &[f64], free: Rectf, out: &mut [Rectf]) -> Rectf {
    let row_area: f64 = row.iter().map(|&i| areas[i]).sum();
    if free.w <= free.h {
        // Horizontal row across the top; thickness grows downward.
        let thickness = row_area / free.w;
        let mut x = free.x;
        for &i in row {
            let w = areas[i] / thickness;
            out[i] = Rectf::new(x, free.y, w, thickness);
            x += w;
        }
        Rectf::new(free.x, free.y + thickness, free.w, (free.h - thickness).max(0.0))
    } else {
        // Vertical column down the left; thickness grows rightward.
        let thickness = row_area / free.h;
        let mut y = free.y;
        for &i in row {
            let h = areas[i] / thickness;
            out[i] = Rectf::new(free.x, y, thickness, h);
            y += h;
        }
        Rectf::new(free.x + thickness, free.y, (free.w - thickness).max(0.0), free.h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn meta(path: &str, size: u64) -> FileMetadata {
        FileMetadata {
            path: PathBuf::from(path),
            size,
            extension: String::new(),
            modified: Some(SystemTime::UNIX_EPOCH),
            accessed: None,
        }
    }

    #[test]
    fn build_aggregates_sizes_up_the_tree() {
        let files = vec![
            meta(r"C:\data\a\1.bin", 100),
            meta(r"C:\data\a\2.bin", 50),
            meta(r"C:\data\b\3.bin", 200),
            meta(r"C:\data\top.bin", 10),
        ];
        let root = build(&files).expect("non-empty");
        assert_eq!(root.path, PathBuf::from(r"C:\data"));
        assert_eq!(root.size, 360);
        // Children sorted largest-first: b (200) before a (150).
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].name, "b");
        assert_eq!(root.children[0].size, 200);
        assert_eq!(root.children[1].name, "a");
        assert_eq!(root.children[1].size, 150);
        // 10 bytes live directly in C:\data (top.bin).
        assert_eq!(root.direct_size(), 10);
    }

    #[test]
    fn squarify_fills_bounds_and_stays_inside() {
        let bounds = Rectf::new(0.0, 0.0, 600.0, 400.0);
        let weights = [6.0, 6.0, 4.0, 3.0, 2.0, 1.0];
        let rects = squarify(&weights, bounds);

        let total_w: f64 = weights.iter().sum();
        let mut covered = 0.0;
        for (i, r) in rects.iter().enumerate() {
            // Each rect lies within the bounds.
            assert!(r.x >= bounds.x - 1e-6 && r.y >= bounds.y - 1e-6);
            assert!(r.x + r.w <= bounds.x + bounds.w + 1e-6);
            assert!(r.y + r.h <= bounds.y + bounds.h + 1e-6);
            // Area is proportional to weight.
            let expected = weights[i] / total_w * bounds.area();
            assert!((r.area() - expected).abs() < 1.0, "tile {i} area off");
            covered += r.area();
        }
        // Tiles cover essentially the whole region.
        assert!((covered - bounds.area()).abs() < 1.0);
    }

    #[test]
    fn squarify_ignores_nonpositive_weights() {
        let rects = squarify(&[0.0, 5.0], Rectf::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(rects[0].area(), 0.0);
        assert!((rects[1].area() - 100.0).abs() < 1e-6);
    }
}
