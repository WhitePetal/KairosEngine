//! The watcher: what it reports and, as important, what it does not.
//!
//! A real backend is the only honest test, so these cases write into a temp
//! directory and poll with a generous deadline: the debounce window is 300 ms and
//! a loaded machine stretches it. The negative cases wait the same window out
//! before concluding that nothing arrived.

use std::fs;
use std::time::{Duration, Instant};

use kairos_editor_core::project::ScanPolicy;

use super::Watcher;

const PATIENCE: Duration = Duration::from_secs(15);

/// Longer than the debounce window plus a margin: after this, a batch the filter
/// dropped cannot still be in flight.
const QUIET: Duration = Duration::from_millis(700);

fn wait_for_change(watcher: &Watcher, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if watcher.take_change() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn start(dir: &tempfile::TempDir) -> Watcher {
    Watcher::new(dir.path(), &ScanPolicy::default()).expect("the watcher starts")
}

#[test]
fn starting_the_watcher_is_not_a_change() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let watcher = start(&dir);

    assert!(!watcher.take_change(), "construction alone must not rescan");
}

/// A file written by another program is the case the editor cannot see any other
/// way.
#[test]
fn a_change_outside_the_editor_is_reported() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let watcher = start(&dir);

    fs::write(dir.path().join("hero.wgsl"), "// hero").expect("write the file");

    assert!(
        wait_for_change(&watcher, PATIENCE),
        "writing a file under the root must reach the watcher"
    );
}

#[test]
fn a_nested_change_is_reported() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let nested = dir.path().join("res/shaders");
    fs::create_dir_all(&nested).expect("the nested directory");
    let watcher = start(&dir);

    fs::write(nested.join("shader.wgsl"), "// shader").expect("write the file");

    assert!(
        wait_for_change(&watcher, PATIENCE),
        "writing a file below the root must reach the watcher"
    );
}

/// The case this watcher adds to the legacy one: a build touches tens of
/// thousands of files under `target/`, and none of them is a reason to rebuild
/// the project tree.
#[test]
fn a_change_in_an_ignored_directory_is_not_reported() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    fs::create_dir_all(dir.path().join("target/debug")).expect("target");
    let watcher = start(&dir);

    fs::write(dir.path().join("target/debug/junk.rlib"), "junk").expect("write the file");
    std::thread::sleep(QUIET);

    assert!(
        !watcher.take_change(),
        "a build artifact must not trigger a project rescan"
    );
}

/// The editor writes its own logs and caches under `Library/` continuously; that
/// is why the system subtree is filtered too (the plan's R4).
#[test]
fn a_change_under_a_system_directory_is_not_reported() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    fs::create_dir_all(dir.path().join("Library/Logs")).expect("Library");
    let watcher = start(&dir);

    fs::write(dir.path().join("Library/Logs/run.log"), "log").expect("write the file");
    std::thread::sleep(QUIET);

    assert!(
        !watcher.take_change(),
        "the editor's own log writes must not trigger a project rescan"
    );
}