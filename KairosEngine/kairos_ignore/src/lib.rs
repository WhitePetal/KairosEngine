//! The project's ignore rules: the pure half of `.kairosignore`.
//!
//! One grammar, two consumers, and this crate is the single implementation of it:
//!
//! * the project browser (`kairos_editor_core::project`) decides which directories
//!   enter the snapshot at all;
//! * the asset pipeline (`kairos_engine::asset_pipeline`) skips the same
//!   directories when it bakes, so what the hierarchy cannot show is never
//!   processed into a product.
//!
//! That sharing is why this is a leaf crate instead of a module in either side:
//! the editor state tree must stay tiny (ADR 0008: no IO, no async), and the
//! engine cannot depend on an editor crate. Neither may own the other's copy of
//! the rules — a panel that hides `res/generated` while the bake processes it is
//! exactly the divergence this crate exists to prevent.
//!
//! The grammar is gitignore's shape, deliberately small:
//!
//! * one rule per line; `#` starts a comment; blank lines are skipped;
//! * a rule without `/` matches a directory **name** at any depth (`target`);
//! * a rule with `/` matches a **path prefix** from the project root
//!   (`res/generated`), and a leading `/` is the same thing;
//! * `*` and `?` work inside one segment; matching is case-insensitive
//!   (Windows/macOS names are);
//! * `!` negation and `**` are rejected in v1 — a rejected rule is reported, not
//!   silently dropped, so a mistyped rule cannot look like it worked.
//!
//! **Rules only ever ignore directories.** Both consumers ask about a directory
//! before descending, so a rule that names a file (`*.tmp`) never matches; dot
//! *files* are hidden by the browser's own rule and skipped by the asset reader,
//! neither of which needs this crate to know about them.

/// The rules that apply when a project has no `.kairosignore`: every dot entry
/// and the Cargo build directory.
///
/// The editor seeds the file with exactly these lines on first open, so writing
/// the file changes nothing until the user edits it.
pub const DEFAULT_IGNORE_LINES: &[&str] = &[".*", "target"];

/// The parsed rules; see the module docs for the grammar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IgnoreRules {
    patterns: Vec<IgnorePattern>,
}

impl IgnoreRules {
    /// The built-in fallback (documented on [`DEFAULT_IGNORE_LINES`]).
    pub fn defaults() -> Self {
        Self::parse(DEFAULT_IGNORE_LINES.iter().map(|line| (*line).to_owned())).0
    }

    /// Parse rule lines, returning the rules and the lines that were rejected.
    ///
    /// Rejected lines are reported rather than dropped: a rule the project cannot
    /// honour must not look like it worked (the same reason a `read_dir` failure
    /// is state, not a log line).
    pub fn parse(lines: impl IntoIterator<Item = String>) -> (Self, Vec<String>) {
        let mut patterns = Vec::new();
        let mut rejected = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match IgnorePattern::parse(line) {
                Some(pattern) => patterns.push(pattern),
                None => rejected.push(line.to_owned()),
            }
        }
        (Self { patterns }, rejected)
    }

    pub fn len(&self) -> usize {
        self.patterns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Whether the directory `rel` should be ignored.
    ///
    /// `rel` is `/`-separated and relative to the project root; `name` is its last
    /// segment, which both callers already have (the walk never needs to split a
    /// path twice) and the watcher passes from the event path.
    pub fn is_ignored(&self, rel: &str, name: &str) -> bool {
        let name = name.to_lowercase();
        self.patterns.iter().any(|pattern| {
            if pattern.segments.len() == 1 {
                return glob_match(&pattern.segments[0], &name);
            }
            let mut actual = rel.split('/');
            pattern.segments.iter().all(|segment| {
                actual
                    .next()
                    .is_some_and(|actual| glob_match(segment, &actual.to_lowercase()))
            })
        })
    }

    /// Whether `path` itself or any of its ancestors is ignored.
    ///
    /// The watcher's question: a change below `target/` must not cost a rescan,
    /// and the event only names the changed file, not the directory the walk
    /// would have skipped.
    pub fn is_ignored_path(&self, path: &str) -> bool {
        if path.is_empty() {
            return false;
        }
        let mut prefix = String::new();
        for segment in path.split('/') {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(segment);
            if self.is_ignored(&prefix, segment) {
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct IgnorePattern {
    /// Lowercased globs, one per path segment. A single segment is a bare name
    /// rule; more than one is a prefix rule.
    segments: Vec<String>,
}

impl IgnorePattern {
    fn parse(line: &str) -> Option<Self> {
        // v1 deliberately does not implement these: rejecting the line is louder
        // than accepting it with a different meaning.
        if line.starts_with('!') || line.contains("**") {
            return None;
        }
        let line = line.trim_start_matches('/');
        // A trailing `/` says "directory only"; every rule is directory-only here,
        // so it is just syntax to absorb.
        let line = line.trim_end_matches('/');
        if line.is_empty() {
            return None;
        }
        let segments: Vec<String> = line
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(str::to_lowercase)
            .collect();
        if segments.is_empty() {
            return None;
        }
        Some(Self { segments })
    }
}

/// Glob match for one path segment: `*` is any run of characters, `?` is one.
///
/// Both sides arrive lowercased, so the comparison is plain char equality.
/// Backtracking is the classic two-pointer form: O(text) per `*` run, no
/// recursion, no allocations beyond the two char vectors.
fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some((p, t));
            p += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}