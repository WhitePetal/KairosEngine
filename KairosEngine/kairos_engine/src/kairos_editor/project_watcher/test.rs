//! Tests for the project tree's watcher.
//!
//! A real filesystem watcher is the only way to test the backend, so these
//! tests write into a temp directory and poll with a generous deadline: the
//! backend's debounce window is 300 ms, and a loaded machine can stretch it.

use std::fs;
use std::time::{Duration, Instant};

use super::ProjectTreeWatcher;

/// Polls `watcher` until it reports a change, panicking once `timeout` passes.
fn wait_for_change(watcher: &ProjectTreeWatcher, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if watcher.take_change() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

const PATIENCE: Duration = Duration::from_secs(15);

/// A file written by another program (the case the editor cannot see any other
/// way) reaches the watcher.
#[test]
fn a_change_outside_the_editor_is_reported() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let watcher = ProjectTreeWatcher::new(dir.path()).expect("the watcher starts");

    // Construction alone reads the tree; it must not report a change, or every
    // editor start would rescan for nothing.
    assert!(
        !watcher.take_change(),
        "starting the watcher is not a change"
    );

    fs::write(dir.path().join("hero.wgsl"), "// hero").expect("write the file");

    assert!(
        wait_for_change(&watcher, PATIENCE),
        "writing a file under the root must reach the watcher"
    );
}

/// A change inside a subdirectory is reported too: the watch is recursive, and
/// the tree shows nested files.
#[test]
fn a_nested_change_is_reported() {
    let dir = tempfile::TempDir::new().expect("a temp dir");
    let nested = dir.path().join("res/shaders");
    fs::create_dir_all(&nested).expect("the nested directory");
    let watcher = ProjectTreeWatcher::new(dir.path()).expect("the watcher starts");

    fs::write(nested.join("shader.wgsl"), "// shader").expect("write the file");

    assert!(
        wait_for_change(&watcher, PATIENCE),
        "writing a file below the root must reach the watcher"
    );
}
