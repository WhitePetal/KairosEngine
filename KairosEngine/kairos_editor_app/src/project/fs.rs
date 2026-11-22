//! The file operations: the only code in the editor that changes the project.
//!
//! Everything here is one call away from the reducer's validated request. The
//! reducer has already checked names, sisters and self-nesting; this module owns
//! what only the file system can answer — whether the path exists, what the OS
//! refuses, and which sidecars travel with a file.
//!
//! Two rules are deliberately stronger than the platforms require:
//!
//! * a rename or move refuses to overwrite: Windows fails on a taken target,
//!   POSIX would clobber it silently, and the editor must behave the same on
//!   both;
//! * a case-only rename goes through a temporary name on **every** platform, so
//!   there is one code path instead of one per file system.
//!
//! A failure returns a one-line message for the panel's status bar. A multi-entry
//! move can fail halfway: the error says so and the completion still requests a
//! rescan, so the tree shows what is actually on disk.

use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use kairos_editor_core::project::{CreateKind, FsOpKind, FsOutcome, RelPath};

/// Execute one operation under [`root`].
///
/// [`companions`] are the sidecar extensions from the type table ("meta"): when an
/// entry is renamed or moved, its existing sidecars move with it (G2/G3).
pub(crate) fn execute(
    root: &Path,
    companions: &[&str],
    kind: &FsOpKind,
) -> Result<FsOutcome, String> {
    match kind {
        FsOpKind::Create { parent, name, kind } => create(root, parent, name, *kind),
        FsOpKind::Rename { from, to } => rename(root, companions, from, to),
        FsOpKind::Move { from, to } => move_entries(root, companions, from, to),
        FsOpKind::Trash { paths } => trash(root, companions, paths),
    }
}

/// The project-relative path as an absolute one, refusing anything that could
/// escape the root.
fn absolute(root: &Path, path: &RelPath) -> Result<PathBuf, String> {
    let mut absolute = root.to_path_buf();
    for segment in path
        .as_str()
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        if segment == "." || segment == ".." {
            return Err(format!("refusing a path outside the project: \"{path}\""));
        }
        absolute.push(segment);
    }
    Ok(absolute)
}

/// An existing project entry as an absolute path.
///
/// The tree's paths are stored lossily (a non-UTF-8 name becomes U+FFFD), so a
/// path that no longer round-trips is reported as unrepresentable instead of
/// being renamed into something else.
fn existing(root: &Path, path: &RelPath) -> Result<PathBuf, String> {
    let absolute = absolute(root, path)?;
    let matches = absolute
        .file_name()
        .is_some_and(|name| name.to_string_lossy() == path.name());
    if !matches {
        return Err(format!(
            "\"{path}\" contains characters the editor cannot represent"
        ));
    }
    Ok(absolute)
}

/// The path with one more extension appended ([`foo.png`] + [`meta`] = [`foo.png.meta`]).
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}

fn create(
    root: &Path,
    parent: &RelPath,
    name: &str,
    kind: CreateKind,
) -> Result<FsOutcome, String> {
    let directory = absolute(root, parent)?;
    let target = directory.join(name);
    match kind {
        CreateKind::Folder => fs::create_dir(&target)
            .map_err(|error| format!("cannot create folder \"{name}\": {error}"))?,
        CreateKind::File { contents, .. } => {
            // [`create_new`] and not [`write`]: a file the tree cannot see (ignored,
            // or created a moment ago from outside) must never be truncated.
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|error| format!("cannot create \"{name}\": {error}"))?;
            file.write_all(contents.as_bytes())
                .map_err(|error| format!("cannot write \"{name}\": {error}"))?;
        }
    }
    Ok(FsOutcome::created(parent.child(name)))
}

fn rename(
    root: &Path,
    companions: &[&str],
    from: &RelPath,
    to: &RelPath,
) -> Result<FsOutcome, String> {
    let source = existing(root, from)?;
    let target = absolute(root, to)?;
    // Sidecars belong to files (see [`trash`]): a directory is one entry, even
    // when a sibling happens to be named like its sidecar. Decided before the
    // rename, while the source still has the kind it was asked about.
    let carries_sidecars = source.is_file();
    rename_entry(&source, &target)?;
    if carries_sidecars {
        for suffix in companions {
            let sidecar = with_suffix(&source, suffix);
            if sidecar.is_file() {
                rename_entry(&sidecar, &with_suffix(&target, suffix))?;
            }
        }
    }
    Ok(FsOutcome::renamed(from.clone(), to.clone()))
}

fn move_entries(
    root: &Path,
    companions: &[&str],
    from: &[RelPath],
    to: &RelPath,
) -> Result<FsOutcome, String> {
    let destination = absolute(root, to)?;
    if !destination.is_dir() {
        return Err(format!("\"{to}\" is not a directory"));
    }
    let mut pairs = Vec::with_capacity(from.len());
    for path in from {
        let source = existing(root, path)?;
        let target = destination.join(path.name());
        // See [`trash`]: only a file carries sidecars, and the kind is read
        // before the move makes the source path meaningless.
        let carries_sidecars = source.is_file();
        rename_entry(&source, &target)?;
        if carries_sidecars {
            for suffix in companions {
                let sidecar = with_suffix(&source, suffix);
                if sidecar.is_file() {
                    rename_entry(&sidecar, &with_suffix(&target, suffix))?;
                }
            }
        }
        pairs.push((path.clone(), to.child(path.name())));
    }
    Ok(FsOutcome::moved(pairs))
}

/// Move entries and their sidecars to the OS recycle bin (D5).
///
/// A delete is a cluster operation like a move: an entry and its `.meta` sidecar
/// must enter the bin together, or deleting an imported texture leaves an
/// orphan sidecar behind (G3).
///
/// Only a file has sidecars: a directory named `foo` and an unrelated file named
/// `foo.meta` are two entries, and deleting the directory must not take the
/// file with it. The same rule governs rename and move.
///
/// A partial failure is reported as a failure; the completion still rescans, so
/// the tree shows what is actually left. There is deliberately no
/// `remove_file` fallback: D5 says a delete the trash cannot take does not
/// happen.
fn trash(root: &Path, companions: &[&str], paths: &[RelPath]) -> Result<FsOutcome, String> {
    let mut targets: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for path in paths {
        let absolute = existing(root, path)?;
        if absolute.is_file() {
            for suffix in companions {
                let sidecar = with_suffix(&absolute, suffix);
                if sidecar.is_file() {
                    targets.push(sidecar);
                }
            }
        }
        targets.push(absolute);
    }
    // One entry per platform call: `trash::delete_all` is a loop over the same
    // call on every backend, and a per-entry call keeps the error message
    // pointing at the entry that failed.
    for target in &targets {
        trash::delete(target).map_err(|error| {
            format!(
                "cannot move \"{}\" to the recycle bin: {error}",
                target.file_name().unwrap_or_default().to_string_lossy()
            )
        })?;
    }
    Ok(FsOutcome::trashed(paths.to_vec()))
}

/// Rename one entry, refusing to overwrite and routing a case-only change
/// through a temporary name.
fn rename_entry(source: &Path, target: &Path) -> Result<(), String> {
    if source == target {
        return Ok(());
    }
    let case_only = source
        .to_string_lossy()
        .to_lowercase()
        .eq(&target.to_string_lossy().to_lowercase());
    // A case-only rename's target "exists" on every case-insensitive file system
    // (it is the same entry), so the overwrite check must not see it.
    if !case_only && target.exists() {
        return Err(format!(
            "\"{}\" already exists",
            target.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    if case_only {
        let temporary = with_suffix(target, "kairos-rename-tmp");
        fs::rename(source, &temporary)
            .map_err(|error| format!("cannot rename \"{}\": {error}", source.display()))?;
        if let Err(error) = fs::rename(&temporary, target) {
            // Put the entry back under its old name rather than stranding it
            // under a temporary one.
            let _ = fs::rename(&temporary, source);
            return Err(format!("cannot rename \"{}\": {error}", source.display()));
        }
        return Ok(());
    }
    fs::rename(source, target).map_err(|error| {
        format!(
            "cannot rename \"{}\" to \"{}\": {error}",
            source.file_name().unwrap_or_default().to_string_lossy(),
            target.file_name().unwrap_or_default().to_string_lossy()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use tempfile::TempDir;

    fn project() -> TempDir {
        let directory = tempfile::tempdir().expect("tempdir");
        fs::create_dir(directory.path().join("res")).expect("res");
        fs::write(directory.path().join("res/a.png"), b"png").expect("a.png");
        fs::write(directory.path().join("res/a.png.meta"), b"meta").expect("a.meta");
        fs::create_dir(directory.path().join("res/models")).expect("models");
        directory
    }

    fn rel(path: &str) -> RelPath {
        RelPath::new(path)
    }

    #[test]
    fn a_folder_is_created_inside_the_parent() {
        let project = project();
        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Create {
                parent: rel("res"),
                name: String::from("New Folder"),
                kind: CreateKind::Folder,
            },
        )
        .expect("created");
        assert_eq!(outcome.created, Some(rel("res/New Folder")));
        assert!(project.path().join("res/New Folder").is_dir());
    }

    #[test]
    fn a_file_template_is_written_once_and_never_overwrites() {
        let project = project();
        let kind = FsOpKind::Create {
            parent: rel("res"),
            name: String::from("New Document.md"),
            kind: CreateKind::File {
                extension: "md",
                contents: "# ",
            },
        };
        execute(project.path(), &["meta"], &kind).expect("created");
        assert_eq!(
            fs::read_to_string(project.path().join("res/New Document.md")).expect("read"),
            "# "
        );

        let again = execute(project.path(), &["meta"], &kind);
        assert!(again.is_err(), "an existing file must never be truncated");
        assert_eq!(
            fs::read_to_string(project.path().join("res/New Document.md")).expect("read"),
            "# ",
            "the first contents survive the refused second create"
        );
    }

    #[test]
    fn a_rename_carries_the_meta_sidecar() {
        let project = project();
        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Rename {
                from: rel("res/a.png"),
                to: rel("res/b.png"),
            },
        )
        .expect("renamed");

        assert_eq!(outcome.moved, vec![(rel("res/a.png"), rel("res/b.png"))]);
        assert!(project.path().join("res/b.png").is_file());
        assert!(project.path().join("res/b.png.meta").is_file());
        assert!(!project.path().join("res/a.png").exists());
        assert!(!project.path().join("res/a.png.meta").exists());
    }

    #[test]
    fn a_case_only_rename_works_through_the_temporary_name() {
        let project = project();
        execute(
            project.path(),
            &["meta"],
            &FsOpKind::Rename {
                from: rel("res/a.png"),
                to: rel("res/A.PNG"),
            },
        )
        .expect("case-only rename");

        let names: Vec<String> = fs::read_dir(project.path().join("res"))
            .expect("read")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert!(names.contains(&String::from("A.PNG")), "{names:?}");
        assert!(names.contains(&String::from("A.PNG.meta")), "{names:?}");
    }

    #[test]
    fn a_rename_refuses_a_taken_target() {
        let project = project();
        fs::write(project.path().join("res/b.png"), b"other").expect("b.png");
        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Rename {
                from: rel("res/a.png"),
                to: rel("res/b.png"),
            },
        );
        assert!(outcome.is_err(), "POSIX would clobber; the editor refuses");
        assert!(project.path().join("res/a.png").is_file());
    }

    #[test]
    fn a_move_moves_every_entry_and_its_sidecar() {
        let project = project();
        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Move {
                from: vec![rel("res/a.png")],
                to: rel("res/models"),
            },
        )
        .expect("moved");

        assert_eq!(
            outcome.moved,
            vec![(rel("res/a.png"), rel("res/models/a.png"))]
        );
        assert!(project.path().join("res/models/a.png").is_file());
        assert!(project.path().join("res/models/a.png.meta").is_file());
    }

    #[test]
    fn a_move_into_a_taken_name_is_refused_before_anything_moves() {
        let project = project();
        fs::write(project.path().join("res/models/a.png"), b"taken").expect("taken");
        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Move {
                from: vec![rel("res/a.png")],
                to: rel("res/models"),
            },
        );
        assert!(outcome.is_err());
        assert!(project.path().join("res/a.png").is_file());
    }

    /// A directory is one entry: a sibling that happens to be named like its
    /// sidecar is an unrelated file and must not be renamed with it.
    #[test]
    fn a_folder_rename_does_not_carry_a_sidecar_named_after_it() {
        let project = project();
        fs::write(project.path().join("res/models.meta"), b"unrelated").expect("sibling");

        execute(
            project.path(),
            &["meta"],
            &FsOpKind::Rename {
                from: rel("res/models"),
                to: rel("res/renamed"),
            },
        )
        .expect("renamed");

        assert!(
            project.path().join("res/models.meta").is_file(),
            "a directory has no sidecars"
        );
        assert!(!project.path().join("res/renamed.meta").exists());
    }

    /// A delete goes to the platform trash and takes the sidecar with it.
    ///
    /// CI may run without a working trash (a headless Linux container without a
    /// trash directory). D5 forbids a permanent-delete fallback, so the correct
    /// behaviour there is that the operation fails and the entries stay — which
    /// is what this test then checks instead of asserting success.
    #[test]
    fn a_delete_takes_the_entry_and_its_sidecar_to_the_trash() {
        let project = project();
        let entry = project.path().join("res/a.png");
        let sidecar = project.path().join("res/a.png.meta");

        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Trash {
                paths: vec![rel("res/a.png")],
            },
        );

        let Ok(outcome) = outcome else {
            eprintln!("skip: the platform trash is unavailable");
            assert!(entry.is_file(), "a failed delete must not remove anything");
            assert!(sidecar.is_file());
            return;
        };

        assert_eq!(outcome.trashed, vec![rel("res/a.png")]);
        assert!(!entry.exists(), "the entry left the project");
        assert!(!sidecar.exists(), "the sidecar travelled with it");
    }

    #[test]
    fn a_trash_refuses_a_path_that_escapes_the_root() {
        let project = project();
        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Trash {
                paths: vec![rel("../outside.png")],
            },
        );
        assert!(outcome.is_err());
    }

    #[test]
    fn a_path_that_escapes_the_root_is_refused() {
        let project = project();
        let outcome = execute(
            project.path(),
            &["meta"],
            &FsOpKind::Create {
                parent: rel(".."),
                name: String::from("x"),
                kind: CreateKind::Folder,
            },
        );
        assert!(outcome.is_err());
    }
}
