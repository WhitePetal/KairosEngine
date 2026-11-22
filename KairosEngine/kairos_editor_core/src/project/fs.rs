//! File-operation requests and the outcomes the host reports back.
//!
//! A request names what to do and where, never how: the reducer builds it from
//! validated state, the app executes it against the file system, and the answer
//! comes back as [`FsOutcome`] — which says *what changed* rather than merely
//! "fine", because the reducer needs the resulting paths to reveal them and to
//! rewrite the view's prefixes after a rename.
//!
//! Every request carries its own [`FsOpId`]. The executor echoes it back, so a
//! late completion can be attributed in the log and a future queued executor can
//! match request to answer; today the reducer only needs the outcome.
//!
//! The operation kinds are the panel's vocabulary, not the file system's: a
//! "move" of several entries is one operation (one [`ProjectAction::ScanRequested`]
//! follows a whole gesture), and a "create" carries the template text the type
//! table supplies ([`CreateKind`]) instead of re-deriving it host-side.

use std::path::PathBuf;

use super::RelPath;

/// Identity of one requested file operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FsOpId(pub u64);

impl FsOpId {
    /// The next id. Wrapping on overflow is fine: ids are for attribution, not
    /// for ordering across more than one outstanding operation.
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

impl std::fmt::Display for FsOpId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fs#{}", self.0)
    }
}

/// One unit of host work: what to do, and under which root.
///
/// The root is repeated from [`ProjectState`](super::ProjectState) because the
/// executor has no access to the state tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FsOp {
    pub id: FsOpId,
    pub root: PathBuf,
    /// Companion extensions from the type table ("meta"): a file's sidecars
    /// travel with it when it is renamed or moved (G2/G3).
    pub companions: Vec<&'static str>,
    pub kind: FsOpKind,
}

/// The operations M3 ships. Their order is the order the panel performs them in
/// when a gesture asks for more than one (a create that is followed by an inline
/// rename is two separate requests, never one compound op).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsOpKind {
    /// Make one entry below the parent.
    ///
    /// The name is the full name (extension included): the reducer appends the
    /// template's extension before asking.
    Create {
        parent: RelPath,
        name: String,
        kind: CreateKind,
    },
    /// Rename one entry in place; the target is a sibling path, never a move.
    Rename { from: RelPath, to: RelPath },
    /// Move entries below one directory, keeping their names.
    Move { from: Vec<RelPath>, to: RelPath },
    /// Move entries to the OS recycle bin (M3, D5).
    ///
    /// Delete is never a permanent removal: the host's implementation is the
    /// platform trash, and a platform whose trash does not work reports a
    /// failure instead of falling back to `remove_file`. A multi-entry delete is
    /// one request for the same reason a multi-entry move is: the panel performs
    /// one gesture, so the protocol answers it once.
    Trash { paths: Vec<RelPath> },
}

/// What a create makes: the two shapes the type table's menu offers.
///
/// The template text rides in the request because the created file's first bytes
/// are part of what the user asked for, not something the executor invents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateKind {
    /// A directory in the parent directory.
    Folder,
    /// One file with the given initial text. The extension is the template's,
    /// so the created row's display comes back from the type table.
    File {
        extension: &'static str,
        contents: &'static str,
    },
}

/// What an operation actually changed.
///
/// Fields rather than an enum because the reducer's work is the same whatever
/// happened: reveal the new path, rewrite the view's prefixes, ask for a scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsOutcome {
    /// The entry a create produced (extension included).
    pub created: Option<RelPath>,
    /// Every entry whose path changed, as `(from, to)`.
    ///
    /// A rename reports one pair, a move of several entries reports each of
    /// them: the reducer rewrites the view's prefixes per pair, which is what
    /// keeps a moved folder expanded and its selection alive.
    pub moved: Vec<(RelPath, RelPath)>,
    /// Every entry that went to the recycle bin.
    ///
    /// Unlike [`Self::moved`] a trashed path has no successor to rewrite the
    /// view to: the follow-up scan prunes the selection and falls the active
    /// directory back to the nearest surviving ancestor. The list is still worth
    /// carrying — it is what the operation log says was removed.
    pub trashed: Vec<RelPath>,
}

impl FsOutcome {
    pub fn created(path: RelPath) -> Self {
        Self {
            created: Some(path),
            ..Self::default()
        }
    }

    pub fn renamed(from: RelPath, to: RelPath) -> Self {
        Self {
            moved: vec![(from, to)],
            ..Self::default()
        }
    }

    pub fn moved(pairs: Vec<(RelPath, RelPath)>) -> Self {
        Self {
            moved: pairs,
            ..Self::default()
        }
    }

    pub fn trashed(paths: Vec<RelPath>) -> Self {
        Self {
            trashed: paths,
            ..Self::default()
        }
    }

    /// The path the follow-up scan should reveal, if any.
    ///
    /// A create and a rename both end with the user expecting to see what they
    /// just made, and a move leaves its last destination as the focus so the
    /// content pane follows the gesture. The reveal only *replaces* a selection
    /// when the focused path is not part of it (see `settle`): a multi-entry
    /// move keeps every moved row selected. A trash reveals nothing on purpose:
    /// the entry is gone, so the only path left to point at would be its parent —
    /// and `settle` would then *select* a folder the user did not ask for. The
    /// pruned selection is the right feedback.
    pub fn focus(&self) -> Option<RelPath> {
        if let Some(created) = &self.created {
            return Some(created.clone());
        }
        self.moved.last().map(|(_, to)| to.clone())
    }
}
