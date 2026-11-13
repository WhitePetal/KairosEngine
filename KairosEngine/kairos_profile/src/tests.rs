//! Capture-pipeline tests: recording lifecycle, frame parsing, analysis,
//! JSON and Chrome-trace export.
//!
//! Puffin's recorder is process-global, so these tests serialise on
//! [`TEST_LOCK`] and reset the global profiler in the fixture. The parser test
//! lives in [`crate::profiler::tests`] because it needs private internals but no
//! global state.

use std::sync::{Mutex, MutexGuard};

use crate::model::{FrameSnapshot, ProfileSession, SampleSnapshot, ThreadSnapshot};
use crate::{Profiler, hierarchy};

static TEST_LOCK: Mutex<()> = Mutex::new(());

struct TestSession {
    _guard: MutexGuard<'static, ()>,
}

impl TestSession {
    fn new() -> Self {
        let guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let profiler = Profiler::global();
        profiler.set_recording(false);
        profiler.clear();
        profiler.set_capacity(crate::DEFAULT_CAPACITY);
        Self { _guard: guard }
    }

    fn profiler(&self) -> &'static Profiler {
        Profiler::global()
    }

    /// Request a capture and let `begin_frame` enable it.
    fn start(&self) {
        let profiler = self.profiler();
        profiler.set_recording(true);
        profiler.begin_frame();
    }

    /// Close the current pass and flush it at the next frame boundary.
    fn commit(&self) {
        let profiler = self.profiler();
        profiler.end_frame(true);
        profiler.begin_frame();
    }
}

impl Drop for TestSession {
    fn drop(&mut self) {
        let profiler = Profiler::global();
        profiler.set_recording(false);
        profiler.clear();
    }
}

#[test]
fn captures_nested_scopes_with_parent_links() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!("test.outer");
        {
            crate::scope!("test.inner", "payload");
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
fn recording_is_off_by_default() {
    let test = TestSession::new();
    assert!(!test.profiler().is_recording());

    test.profiler().begin_frame();
    {
        crate::scope!("test.not.recorded");
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
        crate::scope!("test.discarded");
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
            crate::scope!("test.worker");
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
fn capacity_evicts_the_oldest_frames() {
    let test = TestSession::new();
    test.profiler().set_capacity(2);
    test.start();

    for index in 0..3 {
        {
            let data = index.to_string();
            crate::scope!("test.frame", data);
        }
        test.profiler().end_frame(true);
        test.profiler().begin_frame();
    }

    let session = test.profiler().session();
    assert_eq!(session.frames.len(), 2);
    assert_eq!(session.dropped_frames, 1);
    assert_eq!(
        session.frames[0].source_index + 1,
        session.frames[1].source_index
    );
}

#[test]
fn json_and_chrome_trace_include_the_capture() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!("test.export", "hello");
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
fn write_json_writes_the_capture_to_disk() {
    let test = TestSession::new();
    test.start();
    {
        crate::scope!("test.file");
    }
    test.commit();

    let path = std::env::temp_dir().join(format!(
        "kairos-profile-{}-{}.json",
        std::process::id(),
        test.profiler().capacity()
    ));
    test.profiler().write_json(&path).expect("write capture");
    let json = std::fs::read_to_string(&path).expect("read capture");
    assert!(json.contains("test.file"));
    assert!(json.contains("\"frames\""));
    let _ = std::fs::remove_file(&path);
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
