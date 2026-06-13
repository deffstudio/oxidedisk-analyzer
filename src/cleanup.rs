//! Logic for identifying and cleaning up common Windows temp/cache folders.

use std::path::{Path, PathBuf};
use trash;

#[allow(dead_code)]
pub struct TempFolder {
    pub name: &'static str,
    pub path: PathBuf,
    pub description: &'static str,
}

/// Get a list of common Windows temp folder locations that exist on this system.
pub fn get_known_temp_folders() -> Vec<TempFolder> {
    let mut folders = Vec::new();

    // User Temp
    if let Ok(p) = std::env::var("TEMP") {
        let path = PathBuf::from(p);
        if path.exists() {
            folders.push(TempFolder {
                name: "User Temp",
                path,
                description: "Temporary files created by applications for the current user.",
            });
        }
    }

    // System Temp
    if let Ok(p) = std::env::var("SystemRoot") {
        let mut path = PathBuf::from(p);
        path.push("Temp");
        if path.exists() {
            folders.push(TempFolder {
                name: "System Temp",
                path,
                description: "Windows system-wide temporary files.",
            });
        }
    }

    // Windows Update Cache
    if let Ok(p) = std::env::var("SystemRoot") {
        let mut path = PathBuf::from(p);
        path.push("SoftwareDistribution");
        path.push("Download");
        if path.exists() {
            folders.push(TempFolder {
                name: "Windows Update Cache",
                path,
                description: "Downloaded Windows Update files that have already been installed.",
            });
        }
    }

    // Prefetch
    if let Ok(p) = std::env::var("SystemRoot") {
        let mut path = PathBuf::from(p);
        path.push("Prefetch");
        if path.exists() {
            folders.push(TempFolder {
                name: "Windows Prefetch",
                path,
                description: "Application launch traces used to speed up startup.",
            });
        }
    }

    // Local AppData caches (Browser, etc.)
    if let Ok(p) = std::env::var("LOCALAPPDATA") {
        let root = PathBuf::from(p);
        
        // Thumbnail Cache
        let mut thumb = root.clone();
        thumb.push("Microsoft");
        thumb.push("Windows");
        thumb.push("Explorer");
        if thumb.exists() {
            folders.push(TempFolder {
                name: "Thumbnail Cache",
                path: thumb,
                description: "Cached thumbnails for images and videos.",
            });
        }

        // Edge Cache
        let mut edge = root.clone();
        edge.push("Microsoft");
        edge.push("Edge");
        edge.push("User Data");
        edge.push("Default");
        edge.push("Cache");
        if edge.exists() {
            folders.push(TempFolder {
                name: "Edge Cache",
                path: edge,
                description: "Cached web content from Microsoft Edge.",
            });
        }

        // Chrome Cache
        let mut chrome = root.clone();
        chrome.push("Google");
        chrome.push("Chrome");
        chrome.push("User Data");
        chrome.push("Default");
        chrome.push("Cache");
        if chrome.exists() {
            folders.push(TempFolder {
                name: "Chrome Cache",
                path: chrome,
                description: "Cached web content from Google Chrome.",
            });
        }
    }

    folders
}

/// Critical locations that must never be touched by cleanup, even if a temp
/// scan somehow turns up a path inside them. Resolved from the environment so
/// they're correct regardless of the Windows drive/locale.
pub fn protected_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(sr) = std::env::var("SystemRoot") {
        let win = PathBuf::from(sr);
        roots.push(win.join("System32"));
        roots.push(win.join("SysWOW64"));
        roots.push(win.join("WinSxS"));
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Ok(p) = std::env::var(var) {
            roots.push(PathBuf::from(p));
        }
    }
    roots
}

/// Is `path` inside one of the protected roots? Comparison is case-insensitive
/// with a path-separator boundary so `C:\Windows\System32Foo` does not match
/// `C:\Windows\System32`.
pub fn is_protected(path: &Path, roots: &[PathBuf]) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    roots.iter().any(|root| {
        let r = root.to_string_lossy().to_lowercase();
        p == r || p.starts_with(&format!("{r}\\"))
    })
}

/// Move a file or directory to the Recycle Bin.
pub fn delete_to_trash(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("Failed to recycle {}: {}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_matches_subpaths_case_insensitively_with_boundary() {
        let roots = vec![PathBuf::from(r"C:\Windows\System32")];
        assert!(is_protected(Path::new(r"C:\Windows\System32"), &roots));
        assert!(is_protected(Path::new(r"c:\windows\system32\drivers\etc\hosts"), &roots));
        // Boundary: a sibling dir that merely shares the prefix is not protected.
        assert!(!is_protected(Path::new(r"C:\Windows\System32Foo\x"), &roots));
        assert!(!is_protected(Path::new(r"C:\Windows\Temp\x"), &roots));
    }
}
