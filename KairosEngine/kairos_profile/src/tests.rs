//! Capture-pipeline tests: recording lifecycle, frame parsing, analysis,
//! JSON / Chrome-trace / puffin-native export.
//!
//! Puffin's collector is process-global even though [`Profiler`] is an owned
//! value, so these tests serialise on [`TEST_LOCK`]: only one live recorder may
//! exist at a time. The fixture owns its recorder and lets it drop, which
//! removes the sink and turns the macros off again.

use std::sync::{Mutex, MutexGuard};

use profiling::puffin::FrameView;

use crate::model::{FrameSnapshot, ProfileSession, SampleSnapshot, ThreadSnapshot};
use crate::{Category, Profiler, hierarchy};

static TEST_LOCK: Mutex<()> = Mutex::new(());

struct TestSession {
    profiler: Profiler,
    _guard: MutexGuard<'static, ()>,
}

impl TestSession {
    fn new() -> Self {
        let guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let profiler = Profiler::new();
        Self {
            profiler,
            _guard: guard,
        }
    }

    fn profiler(&self) -> &Profiler {
        &self.profiler
    }

    /// Request a capture and let `begin_frame` enable it.
    fn start(&self) {
        let profiler = self.profiler();
        profiler.set_recording(true);
        profiler.begin_frame();
    }

    /// Close the current frame and flush it at the next frame boundary.
    fn commit(&self) {
        let profiler = self.profiler();
        profiler.end_frame(true);
        profiler.begin_frame();
    }
}

/// A unique scratch directory for one test; removed when `Path` is dropped.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        // The capture tests hold `TEST_LOCK` for their whole body, so only one
        // scratch directory exists at a time and the tag alone is unique.
        let path =
            std::env::temp_dir().join(format!("kairos-profile-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create scratch dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn captures_nested_scopes_with_parent_links() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!(Category::Others, "test.outer");
        {
            crate::scope!(Category::Others, "test.inner", "payload");
        }
    }
    test.commit();

    let session = test.profiler().session();
    assert_eq!(session.frames.len(), 1, "one committed frame");
    let frame = session.frame(0).expect("frame");
    assert_eq!(frame.threads.len(), 1);
    let samples = &frame.threads[0].samples;
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].depth, 0);
    assert_eq!(samples[0].parent, None);
    assert_eq!(samples[1].depth, 1);
    assert_eq!(samples[1].parent, Some(0));
    assert_eq!(session.scope_name(samples[0].scope), "test.outer");
    assert_eq!(session.scope_name(samples[1].scope), "test.inner");
    assert_eq!(samples[1].data.as_deref(), Some("payload"));
    assert!(frame.duration_ns() > 0);
}

#[test]
fn scope_metadata_carries_the_marker_category() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!(Category::Editor, "Kairos.Editor.Panel");
        // Third-party scopes open their markers directly, without a tag: the
        // parser must still classify the known presentation waits.
        crate::profiling::scope!("Surface::get_current_texture");
    }
    test.commit();

    let session = test.profiler().session();
    let editor = session
        .scopes
        .iter()
        .find(|scope| scope.name == "Kairos.Editor.Panel")
        .expect("the tagged marker is in the scope table");
    assert_eq!(editor.category, Category::Editor);

    let vsync = session
        .scopes
        .iter()
        .find(|scope| scope.name == "Surface::get_current_texture")
        .expect("the untagged marker is in the scope table");
    assert_eq!(vsync.category, Category::Vsync);
}

#[test]
fn categories_attribute_exclusive_time_and_skip_the_scheduler_wrapper() {
    let test = TestSession::new();
    test.start();
    {
        // The scheduler wrapper measures its children; its own time must not
        // be attributed anywhere, or the wait and the work would both count.
        crate::scope!(Category::Others, "Kairos.Stage", "Update");
        {
            crate::scope!(
                Category::Others,
                "Kairos.System",
                "Physics/kairos_physics::physics_step_system"
            );
            // An empty scope can land inside one clock tick; the assertion is
            // about attribution, not timing resolution.
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
        {
            crate::scope!(
                Category::Others,
                "Kairos.System",
                "Scripts/kairos_game::spin_system"
            );
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }
    test.commit();

    let session = test.profiler().session();
    let frame = session.frame(0).expect("frame");
    assert!(
        frame.category_ns(Category::Physics) > 0,
        "a physics system folds into Physics"
    );
    assert!(
        frame.category_ns(Category::Scripts) > 0,
        "a game system is Scripts"
    );
    assert_eq!(
        frame.category_ns(Category::Others),
        0,
        "the wrapper's own time is not attributed"
    );
    let attributed: u64 = Category::ALL
        .iter()
        .map(|category| frame.category_ns(*category))
        .sum();
    assert!(
        attributed <= frame.duration_ns(),
        "exclusive time cannot exceed the frame ({attributed} > {})",
        frame.duration_ns()
    );
}

#[test]
fn thread_lanes_are_registered_once_and_kept_in_the_session() {
    let test = TestSession::new();
    test.profiler().register_thread_lane("main");
    test.profiler().register_thread_lane("Compute (0)");
    test.profiler().register_thread_lane("main");

    // Lanes describe threads, not frames: they survive a capture clear.
    test.profiler().clear();
    assert_eq!(
        test.profiler().session().thread_lanes,
        vec!["main".to_owned(), "Compute (0)".to_owned()]
    );
}

#[test]
fn recording_is_off_by_default() {
    let test = TestSession::new();
    assert!(!test.profiler().is_recording());

    test.profiler().begin_frame();
    {
        crate::scope!(Category::Others, "test.not.recorded");
    }
    test.profiler().end_frame(true);
    test.profiler().begin_frame();

    assert!(test.profiler().session().is_empty());
}

#[test]
fn discarded_passes_are_counted_and_not_stored() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!(Category::Others, "test.discarded");
    }
    test.profiler().end_frame(false);
    test.profiler().begin_frame();

    let session = test.profiler().session();
    assert!(session.is_empty());
    assert_eq!(session.discarded_passes, 1);
}

#[test]
fn worker_threads_are_captured_with_names() {
    let test = TestSession::new();
    test.start();

    std::thread::Builder::new()
        .name("profiler-worker".to_owned())
        .spawn(|| {
            crate::scope!(Category::Others, "test.worker");
        })
        .expect("spawn worker")
        .join()
        .expect("join worker");

    test.commit();

    let session = test.profiler().session();
    let frame = session.frame(0).expect("frame");
    let worker = frame
        .threads
        .iter()
        .find(|thread| thread.name == "profiler-worker")
        .expect("worker thread in frame");
    assert!(
        worker
            .samples
            .iter()
            .any(|sample| session.scope_name(sample.scope) == "test.worker")
    );
}

#[test]
fn json_and_chrome_trace_include_the_capture() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!(Category::Others, "test.export", "hello");
    }
    test.commit();

    let profiler = test.profiler();
    let session = profiler.session();

    let json: serde_json::Value = serde_json::from_str(&session.to_json()).expect("session json");
    assert_eq!(json["schemaVersion"], crate::model::SCHEMA_VERSION);
    assert_eq!(json["frames"].as_array().expect("frames").len(), 1);
    let scopes = json["scopes"].as_array().expect("scopes");
    assert!(
        scopes.iter().any(|scope| scope["name"] == "test.export"),
        "scope table names the marker"
    );
    let sample = &json["frames"][0]["threads"][0]["samples"][0];
    assert_eq!(sample["data"], "hello");

    let frame_json: serde_json::Value =
        serde_json::from_str(&profiler.frame_json(0).expect("frame json"))
            .expect("parse frame json");
    assert_eq!(frame_json["schemaVersion"], crate::model::SCHEMA_VERSION);

    let trace: serde_json::Value =
        serde_json::from_str(&profiler.chrome_trace()).expect("trace json");
    let events = trace["traceEvents"].as_array().expect("trace events");
    assert!(
        events
            .iter()
            .any(|event| event["ph"] == "X" && event["name"] == "test.export")
    );
    assert!(
        events
            .iter()
            .any(|event| event["ph"] == "M" && event["name"] == "thread_name")
    );
}

#[test]
fn save_capture_writes_three_files_and_reports_their_paths() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!(Category::Others, "test.file");
    }
    test.commit();

    let dir = TempDir::new("save");
    let files = test
        .profiler()
        .save_capture(dir.path())
        .expect("save capture");

    assert!(
        files.stem.starts_with("capture-") && files.stem.ends_with('Z'),
        "default stem is a UTC timestamp: {}",
        files.stem
    );
    for path in [&files.json, &files.chrome_trace, &files.puffin] {
        assert!(path.exists(), "{} must exist", path.display());
        assert!(
            path.starts_with(dir.path()),
            "{} is under the requested directory",
            path.display()
        );
    }

    let json = std::fs::read_to_string(&files.json).expect("read capture");
    assert!(json.contains("test.file"));
    assert!(json.contains("\"frames\""));

    let trace: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&files.chrome_trace).expect("read trace"))
            .expect("trace json");
    assert!(
        trace["traceEvents"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["ph"] == "X" && event["name"] == "test.file")
    );
}

#[test]
fn puffin_export_is_readable_by_the_backend() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!(Category::Others, "test.puffin.export", "payload");
    }
    test.commit();

    let dir = TempDir::new("puffin");
    let files = test
        .profiler()
        .save_capture_as(dir.path(), "readable")
        .expect("save capture");
    assert_eq!(files.stem, "readable");

    // `FrameView::read` is exactly the reader `puffin_viewer` uses: it checks the
    // `PUF0` magic and then decodes the `PFD4` frame records behind it.
    let mut file = std::fs::File::open(&files.puffin).expect("open puffin capture");
    let view = FrameView::read(&mut file).expect("decode a .puffin capture");
    assert!(!view.is_empty(), "the native capture contains frames");
    assert_eq!(
        view.stats_full().frames(),
        test.profiler().session().frames.len(),
        "every kept frame is in the native capture"
    );
    assert!(
        view.scope_collection()
            .fetch_by_name("Others/test.puffin.export")
            .is_some(),
        "the native capture carries the scope table the viewer resolves names through"
    );
}

fn sample(
    scope: u32,
    start_ns: u64,
    duration_ns: u64,
    depth: u32,
    parent: Option<u32>,
) -> SampleSnapshot {
    SampleSnapshot {
        scope,
        data: None,
        start_ns,
        duration_ns,
        depth,
        parent,
    }
}

fn frame_with(threads: Vec<ThreadSnapshot>) -> FrameSnapshot {
    FrameSnapshot {
        index: 0,
        source_index: 0,
        start_ns: 0,
        end_ns: 100,
        threads,
        categories: [0; crate::Category::COUNT],
    }
}

#[test]
fn hierarchy_aggregates_and_computes_self_time() {
    let frame = frame_with(vec![ThreadSnapshot {
        id: 0,
        name: "main".to_owned(),
        samples: vec![
            sample(1, 0, 100, 0, None),
            sample(2, 10, 30, 1, Some(0)),
            sample(2, 50, 40, 1, Some(0)),
            sample(3, 0, 100, 0, None),
        ],
        warning: None,
    }]);

    let rows = hierarchy(&frame, None);
    assert_eq!(rows.len(), 3);

    // Roots sort by total (tie on scope id): scope 1 before scope 3.
    assert_eq!(rows[0].scope, 1);
    assert_eq!(rows[0].depth, 0);
    assert_eq!(rows[0].total_ns, 100);
    assert_eq!(rows[0].self_ns, 30, "100 - (30 + 40)");
    assert_eq!(rows[0].calls, 1);

    assert_eq!(rows[1].scope, 2);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].total_ns, 70);
    assert_eq!(rows[1].self_ns, 70);
    assert_eq!(rows[1].calls, 2);
    assert_eq!(rows[1].max_ns, 40);

    assert_eq!(rows[2].scope, 3);
    assert_eq!(rows[2].depth, 0);
    assert!(hierarchy(&frame, Some(99)).is_empty());
}

#[test]
fn hierarchy_merges_threads_by_call_path() {
    let frame = frame_with(vec![
        ThreadSnapshot {
            id: 0,
            name: "main".to_owned(),
            samples: vec![sample(1, 0, 100, 0, None)],
            warning: None,
        },
        ThreadSnapshot {
            id: 1,
            name: "worker".to_owned(),
            samples: vec![sample(1, 0, 50, 0, None)],
            warning: None,
        },
    ]);

    let rows = hierarchy(&frame, None);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].scope, 1);
    assert_eq!(rows[0].total_ns, 150);
    assert_eq!(rows[0].self_ns, 150);
    assert_eq!(rows[0].calls, 2);

    let only_worker = hierarchy(&frame, Some(1));
    assert_eq!(only_worker.len(), 1);
    assert_eq!(only_worker[0].total_ns, 50);
}

#[test]
fn profiler_and_session_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Profiler>();
    assert_send_sync::<ProfileSession>();
}
