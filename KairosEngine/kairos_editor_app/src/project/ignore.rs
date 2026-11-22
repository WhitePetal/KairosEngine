//! `.kairosignore`: where the rules come from.
//!
//! The project's ignore file is gitignore-shaped and lives at the project root,
//! so the rules travel with the project and can be committed (D10). This module
//! owns the two IO halves the parser cannot: reading the file, and seeding it
//! with the built-in defaults when it is missing — the same "a fresh checkout
//! just works" behaviour the editor's RON skins already have.

#[cfg(test)]
mod test;

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use kairos_editor_core::project::{DEFAULT_IGNORE_LINES, IgnoreRules};

/// The ignore file's name, relative to the project root.
pub(crate) const IGNORE_FILE: &str = ".kairosignore";

/// Read the project's ignore rules, seeding the file when it is missing.
///
/// A missing file is not an error: it is the common case on a fresh checkout, so
/// the defaults are written in place (once), which makes the rules discoverable
/// and editable instead of hidden in the binary.
pub(crate) fn load_or_seed(root: &Path) -> IgnoreRules {
    let path = root.join(IGNORE_FILE);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let seed = seed_text();
            match fs::write(&path, &seed) {
                Ok(()) => kairos_log::info!(
                    target: "kairos_editor",
                    "project ignore: seeded {} (commit it to share the rules)",
                    path.display()
                ),
                Err(error) => kairos_log::warn!(
                    target: "kairos_editor",
                    "project ignore: cannot seed {}: {error}",
                    path.display()
                ),
            }
            seed
        }
        Err(error) => {
            kairos_log::warn!(
                target: "kairos_editor",
                "project ignore: cannot read {}: {error}",
                path.display()
            );
            return IgnoreRules::defaults();
        }
    };

    let (rules, rejected) = IgnoreRules::parse(text.lines().map(str::to_owned));
    for line in &rejected {
        // A rule the editor cannot honour must not look like it worked: name it
        // and keep the rest of the file usable.
        kairos_log::warn!(
            target: "kairos_editor",
            "project ignore: unsupported rule {line:?} in {} (ignored)",
            path.display()
        );
    }
    rules
}

/// The file written when the project has none.
///
/// Generated from [`DEFAULT_IGNORE_LINES`] so the shipped file and the fallback
/// cannot drift: whether the file exists must not change what is ignored.
pub(crate) fn seed_text() -> String {
    let mut text = String::from(
        r"# .kairosignore — directories the project browser never walks.
# Read when the editor opens the project. Gitignore-shaped, one rule per line:
#   - a rule without '/' matches a directory name at any depth;
#   - a rule with '/' matches a path prefix from the project root;
#   - '*' and '?' work inside one segment; matching is case-insensitive.
# '!' negation and '**' are not supported yet (the editor logs and skips such
# a line).

",
    );
    for line in DEFAULT_IGNORE_LINES {
        text.push_str(line);
        text.push('\n');
    }
    text
}