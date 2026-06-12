//! Filtering and sorting over the in-memory file list.
//!
//! The master `Vec<FileMetadata>` is never mutated; instead these functions
//! produce/order a list of indices into it, which the table renders.

use std::time::{Duration, SystemTime};

use rayon::prelude::*;

use crate::models::{FileMetadata, FilterState, SortKey};

/// Return the indices of files matching `filter`, honoring an optional `top_n`.
pub fn apply(files: &[FileMetadata], filter: &FilterState) -> Vec<usize> {
    let age_cutoff = filter.min_age_days.map(|days| {
        SystemTime::now()
            .checked_sub(Duration::from_secs(days * 24 * 60 * 60))
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });

    let mut indices: Vec<usize> = files
        .par_iter()
        .enumerate()
        .filter(|(_, f)| f.size >= filter.min_size)
        .filter(|(_, f)| match filter.category {
            Some(cat) => f.category() == cat,
            None => true,
        })
        .filter(|(_, f)| match age_cutoff {
            Some(cutoff) => f.modified.map_or(false, |m| m < cutoff),
            None => true,
        })
        .map(|(i, _)| i)
        .collect();

    if let Some(n) = filter.top_n {
        // Largest first, then keep N.
        indices.par_sort_unstable_by(|&a, &b| files[b].size.cmp(&files[a].size));
        indices.truncate(n);
    }

    indices
}

/// Sort `indices` in place by the given key/direction.
pub fn sort(indices: &mut [usize], files: &[FileMetadata], key: SortKey, ascending: bool) {
    indices.par_sort_unstable_by(|&a, &b| {
        let ord = match key {
            SortKey::Size => files[a].size.cmp(&files[b].size),
            SortKey::Name => files[a]
                .name()
                .to_lowercase()
                .cmp(&files[b].name().to_lowercase()),
            SortKey::Modified => files[a].modified.cmp(&files[b].modified),
        };
        if ascending {
            ord
        } else {
            ord.reverse()
        }
    });
}
