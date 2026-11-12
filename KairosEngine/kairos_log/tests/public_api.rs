//! Public surface of the crate: attach a sink, install the logger once, and
//! verify that records — and only the three vocabulary levels — reach it.

use std::sync::{Arc, Mutex};

use kairos_log::{Config, Level, Record, Sink, attach, error, info, init_with, warn};

#[derive(Default)]
struct Collector {
    records: Mutex<Vec<(Level, String, String)>>,
}

impl Sink for Collector {
    fn write(&self, record: &Record) {
        self.records.lock().expect("collector lock").push((
            record.level,
            record.target.clone(),
            record.message.clone(),
        ));
    }
}

#[test]
fn records_reach_attached_sinks_and_debug_is_dropped() {
    let collector = Arc::new(Collector::default());
    let _handle = attach(collector.clone());

    let _guard = init_with(Config {
        level: Level::Info,
        terminal: false,
        file: None,
    })
    .expect("first init installs the logger");
    assert!(
        init_with(Config::default()).is_err(),
        "a second logger cannot be installed"
    );

    info!(target: "kairos_log_test", "hello {}", 1);
    warn!(target: "kairos_log_test", "careful");
    error!(target: "kairos_log_test", "broken");
    log::debug!(target: "kairos_log_test", "not part of the vocabulary");

    let records = collector.records.lock().expect("collector lock");
    assert_eq!(records.len(), 3, "{records:?}");
    assert_eq!(records[0].0, Level::Info);
    assert_eq!(records[1].0, Level::Warning);
    assert_eq!(records[2].0, Level::Error);
    assert_eq!(records[0].2, "hello 1");
    assert!(
        records
            .iter()
            .all(|(_, target, _)| target == "kairos_log_test")
    );
}
