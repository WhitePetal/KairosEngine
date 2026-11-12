//! Headless tests for the console sink: formatting, capacity and the
//! `HostView`-style callback access the panel uses.

use kairos_editor_component::ConsoleLog;
use kairos_log::{Level, Record, Sink};

fn record(level: Level, target: &str, message: &str) -> Record {
    Record::new(level, target, message)
}

#[test]
fn push_formats_records_for_display() {
    let log = ConsoleLog::new();
    log.push(&record(Level::Warning, "kairos_asset", "missing sidecar"));

    log.with_lines(|lines| {
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].level, Level::Warning);
        assert!(
            lines[0]
                .text
                .ends_with("WARN  kairos_asset: missing sidecar"),
            "{}",
            lines[0].text
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
        assert!(lines[0].text.ends_with(": 1"), "{}", lines[0].text);
        assert!(lines[1].text.ends_with(": 2"), "{}", lines[1].text);
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
