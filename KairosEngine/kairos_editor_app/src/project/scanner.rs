//! The walk: one background thread reads directories and reports entries.
//!
//! This is the only place in the project slice that calls `std::fs`, and it is
//! deliberately dumb: a stack of directories, one `read_dir` per directory, no
//! `metadata` except for symlinks. Every *rule* (what is ignored, how deep, how
//! many nodes, which directories are system dirs) lives in
//! `kairos_editor_core::project::ScanPolicy` and is applied here as data.
//!
//! The result is a flat, path-sorted [`ScanEntry`] list. Sorted by bytes is what
//! lets `ProjectTree::assemble` place every parent before its children in one
//! pass, and it is also the order the tree's children are built from.

#[cfg(test)]
mod test;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use kairos_editor_core::project::{RelPath, ScanEntry, ScanPolicy};

/// The walk's answer, ready to become a `ProjectAction::Scanned`.
pub(crate) struct ScanResult {
    pub generation: u64,
    pub entries: Vec<ScanEntry>,
    pub truncated: bool,
    pub elapsed_ms: u64,
    pub library_nodes: usize,
    pub errors: Vec<String>,
}

/// How many unreadable directories to name before the message becomes noise.
const MAX_ERRORS: usize = 8;

/// Walk `root` under `policy`.
///
/// `cancel` is checked once per directory: a watcher storm can request several
/// scans in a row, and every stale one should stop instead of finishing a walk
/// whose result will be thrown away.
pub(crate) fn scan(
    root: &Path,
    policy: &ScanPolicy,
    generation: u64,
    cancel: &AtomicBool,
) -> ScanResult {
    let started = Instant::now();
    let mut entries: Vec<ScanEntry> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut truncated = false;
    let mut library_nodes = 0usize;
    let mut main_nodes = 0usize;

    // (relative path, depth). The root is the empty path at depth 0.
    let mut stack: Vec<(String, u32)> = vec![(String::new(), 0)];
    'walk: while let Some((rel, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let dir = if rel.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&rel)
        };
        let read = match fs::read_dir(&dir) {
            Ok(read) => read,
            Err(error) => {
                record_error(&mut errors, &dir, &error.to_string());
                continue;
            }
        };
        for entry in read {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    record_error(&mut errors, &dir, &error.to_string());
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            // System directories are exempt from the ignore rules: a rule that
            // matches everything must not make `Library` disappear.
            let in_system = is_in_system(&rel, policy)
                || (rel.is_empty() && policy.is_system_dir_name(&name));
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    record_error(&mut errors, &dir, &error.to_string());
                    continue;
                }
            };
            // A directory symlink reads as a folder row but is not followed by
            // default: following links can loop, and the project may contain
            // links the editor did not create.
            let followable = !file_type.is_symlink() || policy.follow_dir_symlinks;
            let is_dir = if file_type.is_symlink() {
                entry.path().metadata().map(|meta| meta.is_dir()).unwrap_or(false)
            } else {
                file_type.is_dir()
            };
            if is_dir && !in_system && policy.ignore.is_ignored(&child, &name)
            {
                continue;
            }

            // Budgets: the system subtree gets its own counter so build output
            // under `Library` cannot eat the main walk's budget.
            if in_system {
                if library_nodes >= policy.system_dir_budget {
                    truncated = true;
                    continue;
                }
            } else if main_nodes >= policy.max_nodes {
                truncated = true;
                break 'walk;
            }

            let descend = is_dir && followable && depth < policy.max_depth;
            if is_dir && followable && !descend {
                // The folder is in the snapshot; its contents are not.
                truncated = true;
            }
            entries.push(ScanEntry {
                path: RelPath::new(child.clone()),
                is_dir,
            });
            if in_system {
                library_nodes += 1;
            } else {
                main_nodes += 1;
            }
            if descend {
                stack.push((child, depth + 1));
            }
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));

    ScanResult {
        generation,
        entries,
        truncated,
        elapsed_ms: started.elapsed().as_millis() as u64,
        library_nodes,
        errors,
    }
}

/// Whether the directory at `rel` is (or is inside) a system directory.
///
/// System directories sit at the project root by construction (see
/// `ScanPolicy::system_dirs`), so the first segment is the whole question.
fn is_in_system(rel: &str, policy: &ScanPolicy) -> bool {
    if rel.is_empty() {
        return false;
    }
    let first = rel.split('/').next().unwrap_or(rel);
    policy.is_system_dir_name(first)
}

/// Aggregate one unreadable directory, capped so a truly broken tree cannot
/// produce a megabyte-long status bar message.
fn record_error(errors: &mut Vec<String>, dir: &Path, message: &str) {
    if errors.len() >= MAX_ERRORS {
        return;
    }
    errors.push(format!("{}: {message}", dir.display()));
}
