//! Per-system scopes: one puffin scope per system *registration site*.
//!
//! The ECS executors open one scope around every system run. The marker stays
//! shared — [`SYSTEM_MARKER`] is the name the Profiler panel and the
//! category-from-data classifier read, and each sample keeps the existing
//! `Tag/system::name` data format — but the scope *id* is per registration
//! site: it is registered with the system's own `file:line` (captured by
//! `#[track_caller]` in `add_systems`), so the capture's
//! [`ScopeInfo::location`](crate::ScopeInfo) points at the code that scheduled
//! the system instead of the executor that ran it.
//!
//! Puffin scope ids are registered on a thread's profiler, so the id cache below
//! is thread-local: a system that runs on several worker threads registers its
//! scope once per thread. The first registration on a thread also emits the
//! scope details in that thread's stream delta, which is how the frame parser
//! fills the session's scope table.
//!
//! Open a scope with [`crate::system_scope!`]. The data string is only built
//! while profiling is on, so a disabled recorder pays one atomic load and no
//! allocation per system run.

use std::{cell::RefCell, collections::HashMap, fmt::Display, panic::Location};

use profiling::puffin::{ProfilerScope, ScopeId, ThreadProfiler};

use crate::category::{Category, SYSTEM_MARKER};

thread_local! {
    /// Puffin scope ids registered by this thread, one per registration site.
    ///
    /// The key is the address of the system's `&'static Location` (a distinct
    /// static per `add_systems` call site). Systems registered without a
    /// location share the fallback key below.
    static SCOPE_IDS: RefCell<HashMap<usize, ScopeId>> = RefCell::new(HashMap::new());
}

/// The cache key for a system's registration site.
///
/// `usize::MAX` never equals a real `&'static Location` address on a 64-bit
/// target (and is only a hash collision risk on a 32-bit one), so it doubles as
/// the "registered without a location" bucket.
const NO_LOCATION: usize = usize::MAX;

fn key(location: Option<&'static Location<'static>>) -> usize {
    location.map_or(NO_LOCATION, |location| {
        location as *const Location<'static> as usize
    })
}

/// The puffin scope id for `location` on this thread, registering it on first
/// use.
fn scope_id(location: Option<&'static Location<'static>>) -> ScopeId {
    let key = key(location);
    if let Some(id) = SCOPE_IDS.with(|ids| ids.borrow().get(&key).copied()) {
        return id;
    }

    // The scope's name is the shared system marker; the function name is empty
    // because the system's own name travels in the per-sample data (the same
    // split the executor used before, where the location was static and only
    // the system name varied).
    let (file, line) = location.map_or(("", 0), |location| (location.file(), location.line()));
    let id =
        ThreadProfiler::call(|thread| thread.register_named_scope(SYSTEM_MARKER, "", file, line));
    SCOPE_IDS.with(|ids| {
        ids.borrow_mut().insert(key, id);
    });
    id
}

/// The guard for one system-purpose scope; ends the puffin scope when dropped.
///
/// Like the guards the `scope!` macros produce it must be created and dropped on
/// the same thread (puffin scopes are thread-local); it is intentionally not
/// `Send`.
pub struct SystemScope {
    _scope: ProfilerScope,
}

/// Begin a per-system scope on the calling thread, if scopes are on.
///
/// `category` and `system` are composed into the sample's data as
/// `Tag/system::name`, the format the panel parses:
///
/// * the category tag is the module the system declared (or `Scripts`);
/// * the name is the system's full type path, which is also its display name.
///
/// `location` is the system's registration site, captured with
/// `#[track_caller]` when it was added to a schedule; `None` registers a
/// fallback scope with no location (the executor passes its own call site in
/// that case).
///
/// Returns `None` when the recorder is off. Use [`crate::system_scope!`]
/// rather than calling this directly: the macro skips evaluating the data
/// arguments while profiling is off.
pub fn begin(
    category: Category,
    system: impl Display,
    location: Option<&'static Location<'static>>,
) -> Option<SystemScope> {
    if !profiling::puffin::are_scopes_on() {
        return None;
    }
    let data = format!("{}/{}", category.tag(), system);
    Some(SystemScope {
        _scope: ProfilerScope::new(scope_id(location), data),
    })
}

/// Open a per-system profiling scope until the end of the enclosing block.
///
/// The marker is the shared [`SYSTEM_MARKER`] — the panel's per-sample data path
/// and its category-from-data rule stay unchanged — while the scope's recorded
/// location is the system's own registration site (`file:line`), not the
/// executor's call site. The sample's data is `Tag/system::name`, built only
/// while profiling is on.
///
/// `location` is an `Option<&'static std::panic::Location<'static>>`:
/// `Some` is the location captured by `add_systems`, `None` is a system that
/// was inserted without one.
///
/// The executor opens it around each system run:
///
/// ```ignore
/// let location = system.profiler_location().unwrap_or_else(|| Location::caller());
/// kairos_profile::system_scope!(system.profiler_category(), system.name(), Some(location));
/// ```
#[macro_export]
macro_rules! system_scope {
    ($category:expr, $name:expr, $location:expr) => {
        let _system_scope = if $crate::profiling::puffin::are_scopes_on() {
            $crate::system_scope::begin($category, $name, $location)
        } else {
            None
        };
    };
}
