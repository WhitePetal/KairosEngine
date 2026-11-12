//! Stack-trace shaping: internal/noise frames must not reach the console.

use std::sync::{Arc, Mutex};

use kairos_log::{Config, Level, Record, Sink, attach, info, init_with};

#[derive(Clone)]
struct Collector(Arc<Mutex<Vec<Record>>>);

impl Sink for Collector {
    fn write(&self, record: &Record) {
        self.0.lock().expect("lock").push(record.clone());
    }
}

#[test]
fn stack_trace_keeps_caller_frames_only() {
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
    let stack = record
        .stack_trace()
        .expect("RUST_BACKTRACE=1 tests capture a stack");
    println!("--- stack ---\n{stack}--- end ---");
    assert!(
        stack.contains("stack_trace_keeps_caller_frames_only"),
        "{stack}"
    );
    assert!(!stack.contains("winit::"), "{stack}");
    assert!(!stack.contains("eframe::"), "{stack}");
    assert!(!stack.contains("kairos_log::"), "{stack}");
    assert!(
        !stack.contains("FnOnce"),
        "trampolines are not caller frames: {stack}"
    );
    assert!(
        !stack.contains("test::"),
        "test harness frames are trimmed: {stack}"
    );
    assert!(
        !stack.contains("pthread"),
        "runtime tail frames are trimmed: {stack}"
    );
}
