//! Stack-trace shaping: internal/noise frames must not reach the console.

use std::sync::{Arc, Mutex};

use kairos_log::{Config, Level, Record, Sink, StackFrame, attach, info, init_with};

#[derive(Clone)]
struct Collector(Arc<Mutex<Vec<Record>>>);

impl Sink for Collector {
    fn write(&self, record: &Record) {
        self.0.lock().expect("lock").push(record.clone());
    }
}

/// A frame as the console renders it, for failure messages.
fn render(frame: &StackFrame) -> String {
    match &frame.source {
        Some(source) => format!("{} (at {source})", frame.symbol),
        None => frame.symbol.clone(),
    }
}

#[test]
fn stack_frames_keep_caller_frames_only() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let sink = Collector(records.clone());
    let _handle = attach(Arc::new(sink));
    let _guard = init_with(Config {
        level: Level::Info,
        terminal: false,
        file: None,
    })
    .expect("logger installed");

    info!(target: "kairos_log_test", "stack probe");

    let record = records.lock().expect("lock")[0].clone();
    let frames = record.stack_frames();
    let stack = frames.iter().map(render).collect::<Vec<_>>().join("\n");
    println!("--- stack ---\n{stack}\n--- end ---");

    let caller = "stack_frames_keep_caller_frames_only";
    assert!(
        frames.iter().any(|frame| frame.symbol.contains(caller)),
        "{stack}"
    );

    // Runtime, host-framework and logger frames are not caller frames.
    for noise in ["winit::", "eframe::", "kairos_log::"] {
        assert!(
            frames.iter().all(|frame| !frame.symbol.contains(noise)),
            "{noise} reached the console: {stack}"
        );
    }
    assert!(
        frames.iter().all(|frame| !frame.symbol.contains("FnOnce")),
        "trampolines are not caller frames: {stack}"
    );
    assert!(
        frames.iter().all(|frame| !frame.symbol.contains("test::")),
        "test harness frames are trimmed: {stack}"
    );
    assert!(
        frames.iter().all(|frame| !frame.symbol.contains("pthread")),
        "runtime tail frames are trimmed: {stack}"
    );

    // The console paints a frame's source position as the link, so a location
    // the toolchain resolved must survive shaping as a typed source. Windows
    // resolves it without a column — the format that used to be dropped.
    // Builds without line info (no `at` lines in the raw trace) have no
    // source to keep.
    let raw = record
        .stack
        .as_ref()
        .expect("captured backtrace")
        .to_string();
    if raw.contains(" at ") {
        assert!(
            frames.iter().any(|frame| frame.source.is_some()),
            "source locations survive shaping: {stack}"
        );
    }
}
