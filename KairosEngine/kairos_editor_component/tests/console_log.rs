//! Headless tests for the console sink: structured entries, capacity and the
//! `HostView`-style callback access the panel uses.

use kairos_editor_component::ConsoleLog;
use kairos_log::{Level, Record, Sink, SourceLocation, StackFrame};

fn record(level: Level, target: &str, message: &str) -> Record {
    Record::new(level, target, message)
}

#[test]
fn push_keeps_time_level_and_message_separate() {
    let log = ConsoleLog::new();
    log.push(&record(Level::Warning, "kairos_asset", "missing sidecar"));

    log.with_lines(|lines| {
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].level, Level::Warning);
        assert_eq!(lines[0].message, "missing sidecar");
        assert_eq!(lines[0].target, "kairos_asset");
        assert_eq!(lines[0].time.len(), 8, "HH:MM:SS");
        assert!(lines[0].stack.is_empty(), "no backtrace in a bare Record");
    });
}

#[test]
fn entries_get_monotonic_ids() {
    let log = ConsoleLog::new();
    log.push(&record(Level::Info, "test", "one"));
    log.push(&record(Level::Info, "test", "two"));

    assert_eq!(log.last_id(), Some(2));
    log.with_lines(|lines| {
        assert_eq!(lines[0].id, 1);
        assert_eq!(lines[1].id, 2);
    });
}

#[test]
fn source_location_is_the_stack_fallback() {
    let log = ConsoleLog::new();
    let mut record = record(Level::Error, "kairos_physics", "fixed step failed");
    record.file = Some("src/physics.rs".to_owned());
    record.line = Some(42);
    log.push(&record);

    log.with_lines(|lines| {
        assert_eq!(lines[0].location.as_deref(), Some("src/physics.rs:42"));
        assert_eq!(
            lines[0].stack,
            [StackFrame {
                symbol: "kairos_physics".to_owned(),
                source: Some(SourceLocation {
                    file: "src/physics.rs".to_owned(),
                    line: Some(42),
                }),
            }]
        );
    });
}

#[test]
fn capacity_keeps_the_newest_lines() {
    let log = ConsoleLog::with_capacity(2);
    for index in 0..3 {
        log.push(&record(Level::Info, "test", &index.to_string()));
    }

    assert_eq!(log.len(), 2);
    log.with_lines(|lines| {
        assert_eq!(lines[0].message, "1");
        assert_eq!(lines[1].message, "2");
    });
}

#[test]
fn capacity_is_at_least_one_line() {
    let log = ConsoleLog::with_capacity(0);
    log.push(&record(Level::Info, "test", "kept"));
    assert_eq!(log.len(), 1);
}

#[test]
fn sink_impl_writes_into_the_buffer() {
    let log = ConsoleLog::new();
    let sink: &dyn Sink = &log;
    sink.write(&record(Level::Error, "kairos_physics", "fixed step failed"));

    assert_eq!(log.len(), 1);
    log.with_lines(|lines| assert_eq!(lines[0].level, Level::Error));
}

#[test]
fn clear_and_is_empty() {
    let log = ConsoleLog::new();
    assert!(log.is_empty());
    log.push(&record(Level::Info, "test", "hello"));
    assert!(!log.is_empty());
    log.clear();
    assert!(log.is_empty());
}
