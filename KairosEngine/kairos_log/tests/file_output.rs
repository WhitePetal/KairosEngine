//! End-to-end: `init_with` creates the configured log file and the process
//! logger writes every record into it (this test binary owns its own process,
//! so it can install the one global logger).

use kairos_log::{Config, Level, error, info, init_with};

#[test]
fn init_creates_the_log_file_and_writes_records() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("Logs").join("KairosEngine.log");

    {
        let _guard = init_with(Config {
            level: Level::Info,
            terminal: false,
            file: Some(path.clone()),
        })
        .expect("logger installed");

        info!(target: "kairos_log_test", "editor booted");
        error!(target: "kairos_log_test", "render pipeline failed");
        // Dropping the guard flushes the file sink.
    }

    let written = std::fs::read_to_string(&path).expect("log file");
    assert!(
        written.contains("INFO  kairos_log_test: editor booted"),
        "{written}"
    );
    assert!(
        written.contains("ERROR kairos_log_test: render pipeline failed"),
        "{written}"
    );
}
