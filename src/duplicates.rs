//! Background duplicate detection using blake3.
//!
//! Three-stage funnel to avoid hashing files that can't possibly match:
//!   1. Group by exact size — only same-size files can be duplicates.
//!   2. For size collisions, hash the first 16 KB (cheap) and regroup.
//!   3. For prefix collisions, hash the full file to confirm.
//!
//! Runs on its own thread and streams [`DupMessage`]s to the UI, processing
//! candidates in chunks so progress updates and repaints happen periodically.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;

use rayon::prelude::*;

use crate::models::{DupMessage, DuplicateGroup, FileMetadata};

/// Bytes hashed in the cheap prefix stage.
const PREFIX_LIMIT: u64 = 16 * 1024;
/// Candidates hashed per chunk between progress updates.
const CHUNK: usize = 256;

type Digest = [u8; 32];

pub fn spawn_find(files: Arc<Vec<FileMetadata>>, tx: Sender<DupMessage>, ctx: egui::Context) {
    thread::spawn(move || {
        // Stage 1: group by size; only groups of 2+ (and non-empty files) can collide.
        let mut by_size: HashMap<u64, Vec<usize>> = HashMap::new();
        for (i, f) in files.iter().enumerate() {
            if f.size > 0 {
                by_size.entry(f.size).or_default().push(i);
            }
        }
        let candidates: Vec<usize> = by_size
            .into_values()
            .filter(|v| v.len() >= 2)
            .flatten()
            .collect();

        let total = candidates.len();
        if total == 0 {
            let _ = tx.send(DupMessage::Done(Vec::new()));
            ctx.request_repaint();
            return;
        }

        // Stage 2: prefix hash, keyed by (size, prefix-digest) so equal prefixes
        // of different sizes never share a bucket.
        let prefix_hashes = hash_chunked(&files, &candidates, &tx, &ctx, total, |path| {
            hash_file(path, Some(PREFIX_LIMIT))
        });
        let mut by_prefix: HashMap<(u64, Digest), Vec<usize>> = HashMap::new();
        for (i, digest) in prefix_hashes {
            by_prefix.entry((files[i].size, digest)).or_default().push(i);
        }
        let confirm: Vec<usize> = by_prefix
            .into_values()
            .filter(|v| v.len() >= 2)
            .flatten()
            .collect();

        // Stage 3: full hash to confirm. Group by full digest (size already matches).
        let full_hashes = hash_chunked(&files, &confirm, &tx, &ctx, confirm.len(), |path| {
            hash_file(path, None)
        });
        let mut by_full: HashMap<Digest, Vec<usize>> = HashMap::new();
        for (i, digest) in full_hashes {
            by_full.entry(digest).or_default().push(i);
        }

        let mut groups: Vec<DuplicateGroup> = by_full
            .into_iter()
            .filter(|(_, v)| v.len() >= 2)
            .map(|(digest, members)| DuplicateGroup {
                size: files[members[0]].size,
                hash: to_hex(&digest),
                members,
            })
            .collect();
        // Biggest reclaimable savings first.
        groups.sort_unstable_by(|a, b| b.wasted().cmp(&a.wasted()));

        let _ = tx.send(DupMessage::Done(groups));
        ctx.request_repaint();
    });
}

/// Hash each index in `items` in parallel chunks, reporting progress as
/// `base + processed` of `total`. IO errors are sent as `DupMessage::Error`
/// and the file is dropped from the results.
fn hash_chunked(
    files: &[FileMetadata],
    items: &[usize],
    tx: &Sender<DupMessage>,
    ctx: &egui::Context,
    total: usize,
    hash_fn: impl Fn(&Path) -> io::Result<Digest> + Sync,
) -> Vec<(usize, Digest)> {
    let mut out: Vec<(usize, Digest)> = Vec::with_capacity(items.len());
    let mut done = 0usize;

    for chunk in items.chunks(CHUNK) {
        let results: Vec<(usize, io::Result<Digest>)> = chunk
            .par_iter()
            .map(|&i| (i, hash_fn(&files[i].path)))
            .collect();

        for (i, res) in results {
            match res {
                Ok(d) => out.push((i, d)),
                Err(e) => {
                    let _ = tx.send(DupMessage::Error(format!(
                        "{}: {}",
                        files[i].path.display(),
                        e
                    )));
                }
            }
        }

        done += chunk.len();
        let _ = tx.send(DupMessage::Progress {
            hashed: done,
            total,
        });
        ctx.request_repaint();
    }

    out
}

/// blake3 the file at `path`, optionally only the first `limit` bytes.
fn hash_file(path: &Path, limit: Option<u64>) -> io::Result<Digest> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    let mut remaining = limit.unwrap_or(u64::MAX);

    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let n = file.read(&mut buf[..want])?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        remaining -= n as u64;
    }

    Ok(*hasher.finalize().as_bytes())
}

fn to_hex(bytes: &Digest) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_temp(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("oxidedisk_test_{}_{}", std::process::id(), name));
        fs::write(&p, contents).unwrap();
        p
    }

    #[test]
    fn identical_content_hashes_equal_distinct_differs() {
        let a = write_temp("a", b"the quick brown fox");
        let b = write_temp("b", b"the quick brown fox");
        let c = write_temp("c", b"the quick brown cat");

        let ha = hash_file(&a, None).unwrap();
        let hb = hash_file(&b, None).unwrap();
        let hc = hash_file(&c, None).unwrap();

        assert_eq!(ha, hb, "identical files must hash equal");
        assert_ne!(ha, hc, "different files must hash differently");

        for p in [a, b, c] {
            let _ = fs::remove_file(p);
        }
    }

    #[test]
    fn prefix_limit_only_reads_first_n_bytes() {
        // Same first 8 bytes, different tails.
        let a = write_temp("p1", b"PREFIX01_tailA");
        let b = write_temp("p2", b"PREFIX01_tailB_longer");

        let pa = hash_file(&a, Some(8)).unwrap();
        let pb = hash_file(&b, Some(8)).unwrap();
        assert_eq!(pa, pb, "equal 8-byte prefixes must hash equal");

        let fa = hash_file(&a, None).unwrap();
        let fb = hash_file(&b, None).unwrap();
        assert_ne!(fa, fb, "full hashes of different files must differ");

        for p in [a, b] {
            let _ = fs::remove_file(p);
        }
    }
}
