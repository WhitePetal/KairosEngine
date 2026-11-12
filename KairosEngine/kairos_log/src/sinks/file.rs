use std::{
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::Path,
    sync::{Mutex, MutexGuard, PoisonError},
};

use crate::{Level, Record, Sink};

const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Appends records to a log file, creating parent directories at init.
///
/// `kairos_log::init_with` creates (and truncates) the file, so each run leaves
/// exactly one readable log. Writes are buffered; `Error` records flush
/// immediately so a crash report still has them, and the process guard flushes
/// the rest on exit.
pub(crate) struct FileSink {
    file: Mutex<BufWriter<File>>,
}

impl FileSink {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        Ok(Self {
            file: Mutex::new(BufWriter::new(File::create(path)?)),
        })
    }

    fn file(&self) -> MutexGuard<'_, BufWriter<File>> {
        self.file.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Sink for FileSink {
    fn write(&self, record: &Record) {
        let mut file = self.file();
        if writeln!(file, "{}", record.line(TIME_FORMAT)).is_err() {
            return;
        }
        if record.level == Level::Error {
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        let _ = self.file().flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Level;

    #[test]
    fn creates_parent_directories_and_writes_the_line() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("logs").join("kairos.log");

        let sink = FileSink::open(&path).expect("open");
        sink.write(&Record::new(
            Level::Warning,
            "kairos_asset",
            "missing sidecar",
        ));
        sink.flush();

        let written = fs::read_to_string(&path).expect("read log");
        assert!(
            written.contains(" WARN  kairos_asset: missing sidecar"),
            "{written}"
        );
    }

    #[test]
    fn error_records_are_flushed_immediately() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("kairos.log");
        let sink = FileSink::open(&path).expect("open");

        sink.write(&Record::new(
            Level::Error,
            "kairos_physics",
            "fixed step failed",
        ));

        let written = fs::read_to_string(&path).expect("read log");
        assert!(written.contains("fixed step failed"), "{written}");
    }
}
