use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn scan_repos<P: AsRef<Path>>(root: P, max_depth: usize) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    let walker = WalkDir::new(root).max_depth(max_depth);

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_dir() && entry.file_name() == ".git" {
            // Found a .git directory, push its parent
            if let Some(parent) = entry.path().parent() {
                repos.push(parent.to_path_buf());
            }
        }
    }
    repos
}
