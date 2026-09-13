//! The asset processor's write-ahead log.
//!
//! This is the port of `bevy_asset` 0.19.1's `processor/log.rs` that kairos's
//! rewrite deferred (ADR 0005). It makes processing **transactional**: before an
//! asset's processed bytes are written, a `Begin` entry is flushed to the log;
//! after both the bytes and the `.meta` sidecar are written, an `End` entry is
//! flushed. A `Begin` with no matching `End` means the previous run crashed (or
//! failed) mid-write, so the processed output may be torn.
//!
//! On startup [`validate_transaction_log`] reads the previous log and classifies
//! every entry against that contract. The processor reacts to each outcome:
//!
//! - an unfinished transaction has its processed bytes and `.meta` deleted, so
//!   the ordinary initial pass regenerates them;
//! - a log that is unreadable or not a valid begin/end sequence invalidates the
//!   whole processed folder, forcing a full rebuild rather than serving partial
//!   output.
//!
//! [`FileTransactionLogFactory`] is the default store: one line per entry in
//! `<base>/imported_assets/log`, each flushed immediately. Tests inject their own
//! [`ProcessorTransactionLogFactory`] through
//! [`AssetProcessorData::set_log_factory`](super::AssetProcessorData::set_log_factory)
//! so they never touch the filesystem.

use std::io::ErrorKind;
use std::path::PathBuf;

use async_fs::File;
use futures_lite::{AsyncReadExt, AsyncWriteExt};
use kairos_collections::FixedHashSet;
use kairos_ecs::error::KairosError;
use kairos_tasks::BoxedFuture;
use thiserror::Error;

use crate::path::AssetPath;

/// An in-memory representation of a single [`ProcessorTransactionLog`] entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEntry {
    /// An asset started processing.
    BeginProcessing(AssetPath<'static>),
    /// An asset finished processing.
    EndProcessing(AssetPath<'static>),
    /// An unrecoverable error was logged; every asset must be reprocessed.
    UnrecoverableError,
}

/// A factory of [`ProcessorTransactionLog`] that handles the state before the log has been started.
///
/// This trait also assists in recovering from partial processing by fetching the previous state of
/// the transaction log.
pub trait ProcessorTransactionLogFactory: Send + Sync + 'static {
    /// Reads all entries in a previous transaction log if present.
    ///
    /// If there is no previous transaction log, this method should return an empty Vec of entries.
    fn read(&self) -> BoxedFuture<'_, Result<Vec<LogEntry>, KairosError>>;

    /// Creates a new transaction log to write to.
    ///
    /// This should remove any previous entries if they exist.
    fn create_new_log(
        &self,
    ) -> BoxedFuture<'_, Result<Box<dyn ProcessorTransactionLog>, KairosError>>;
}

/// A "write ahead" logger that helps ensure asset processing is transactional.
///
/// Prior to processing an asset, we write to the log to indicate it has started. After processing
/// an asset, we write to the log to indicate it has finished. On startup, the log can be read
/// through [`ProcessorTransactionLogFactory`] to determine if any transactions were incomplete.
pub trait ProcessorTransactionLog: Send + Sync + 'static {
    /// Logs the start of an asset being processed.
    ///
    /// If this is not followed at some point in the log by a closing
    /// [`ProcessorTransactionLog::end_processing`], in the next run of the processor the asset
    /// processing will be considered "incomplete" and it will be reprocessed.
    fn begin_processing<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), KairosError>>;

    /// Logs the end of an asset being successfully processed. See
    /// [`ProcessorTransactionLog::begin_processing`].
    fn end_processing<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), KairosError>>;

    /// Logs an unrecoverable error.
    ///
    /// On the next run of the processor, all assets will be regenerated. This should only be used
    /// as a last resort. Every call to this should be considered with scrutiny and ideally replaced
    /// with something more granular.
    fn unrecoverable(&mut self) -> BoxedFuture<'_, Result<(), KairosError>>;
}

/// Validates the previous state of the transaction log and determines any assets that need to be
/// reprocessed.
pub(crate) async fn validate_transaction_log(
    log_factory: &dyn ProcessorTransactionLogFactory,
) -> Result<(), ValidateLogError> {
    let mut transactions: FixedHashSet<AssetPath<'static>> = FixedHashSet::default();
    let mut errors: Vec<LogEntryError> = Vec::new();
    let entries = log_factory
        .read()
        .await
        .map_err(ValidateLogError::ReadLogError)?;
    for entry in entries {
        match entry {
            LogEntry::BeginProcessing(path) => {
                // Every start should be followed by:
                //   * nothing (if there was an abrupt stop), or
                //   * an end (if the transaction completed).
                if !transactions.insert(path.clone()) {
                    errors.push(LogEntryError::DuplicateTransaction(path));
                }
            }
            LogEntry::EndProcessing(path) => {
                if !transactions.remove(&path) {
                    errors.push(LogEntryError::EndedMissingTransaction(path));
                }
            }
            LogEntry::UnrecoverableError => return Err(ValidateLogError::UnrecoverableError),
        }
    }
    for transaction in transactions {
        errors.push(LogEntryError::UnfinishedTransaction(transaction));
    }
    if !errors.is_empty() {
        return Err(ValidateLogError::EntryErrors(errors));
    }
    Ok(())
}

/// A transaction log factory that uses a file as its storage.
pub struct FileTransactionLogFactory {
    /// The file path that the transaction log should write to.
    pub file_path: PathBuf,
}

/// The log path relative to the base path, matching upstream.
const LOG_PATH: &str = "imported_assets/log";

impl Default for FileTransactionLogFactory {
    fn default() -> Self {
        let file_path = crate::io::file::get_base_path().join(LOG_PATH);
        Self { file_path }
    }
}

impl ProcessorTransactionLogFactory for FileTransactionLogFactory {
    fn read(&self) -> BoxedFuture<'_, Result<Vec<LogEntry>, KairosError>> {
        let path = self.file_path.clone();
        Box::pin(async move {
            let mut log_lines = Vec::new();
            let mut file = match File::open(path).await {
                Ok(file) => file,
                Err(err) => {
                    if err.kind() == ErrorKind::NotFound {
                        // If the log file doesn't exist, this is equivalent to an empty file.
                        return Ok(log_lines);
                    }
                    return Err(err.into());
                }
            };
            let mut string = String::new();
            file.read_to_string(&mut string).await?;
            for line in string.lines() {
                if let Some(path_str) = line.strip_prefix(ENTRY_BEGIN) {
                    log_lines.push(LogEntry::BeginProcessing(
                        AssetPath::parse(path_str).into_owned(),
                    ));
                } else if let Some(path_str) = line.strip_prefix(ENTRY_END) {
                    log_lines.push(LogEntry::EndProcessing(
                        AssetPath::parse(path_str).into_owned(),
                    ));
                } else if line.is_empty() {
                    continue;
                } else {
                    return Err(ReadLogError::InvalidLine(line.to_string()).into());
                }
            }
            Ok(log_lines)
        })
    }

    fn create_new_log(
        &self,
    ) -> BoxedFuture<'_, Result<Box<dyn ProcessorTransactionLog>, KairosError>> {
        let path = self.file_path.clone();
        Box::pin(async move {
            // A missing log file is the expected fresh-start case; any other
            // removal failure is harmless because `File::create` truncates the
            // path anyway.
            let _ = async_fs::remove_file(&path).await;

            if let Some(parent_folder) = path.parent() {
                async_fs::create_dir_all(parent_folder).await?;
            }

            Ok(Box::new(FileProcessorTransactionLog {
                log_file: File::create(path).await?,
            }) as Box<dyn ProcessorTransactionLog>)
        })
    }
}

/// A file-backed [`ProcessorTransactionLog`]: one flushed line per entry.
struct FileProcessorTransactionLog {
    /// The file to write logs to.
    log_file: File,
}

impl FileProcessorTransactionLog {
    /// Writes `line` to the file and flushes it.
    async fn write(&mut self, line: &str) -> Result<(), KairosError> {
        self.log_file.write_all(line.as_bytes()).await?;
        self.log_file.flush().await?;
        Ok(())
    }
}

const ENTRY_BEGIN: &str = "Begin ";
const ENTRY_END: &str = "End ";
const UNRECOVERABLE_ERROR: &str = "UnrecoverableError";

impl ProcessorTransactionLog for FileProcessorTransactionLog {
    fn begin_processing<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), KairosError>> {
        Box::pin(async move { self.write(&format!("{ENTRY_BEGIN}{asset}\n")).await })
    }

    fn end_processing<'a>(
        &'a mut self,
        asset: &'a AssetPath<'_>,
    ) -> BoxedFuture<'a, Result<(), KairosError>> {
        Box::pin(async move { self.write(&format!("{ENTRY_END}{asset}\n")).await })
    }

    fn unrecoverable(&mut self) -> BoxedFuture<'_, Result<(), KairosError>> {
        Box::pin(async move { self.write(UNRECOVERABLE_ERROR).await })
    }
}

/// An error that occurs when reading from the [`ProcessorTransactionLog`] fails.
#[derive(Error, Debug)]
pub enum ReadLogError {
    /// An invalid log line was encountered, consisting of the contained string.
    #[error("Encountered an invalid log line: '{0}'")]
    InvalidLine(String),
    /// A file-system-based error occurred while reading the log file.
    #[error("Failed to read log file: {0}")]
    Io(#[from] std::io::Error),
}

/// An error that occurs when writing to the [`ProcessorTransactionLog`] fails.
#[derive(Error, Debug)]
#[error(
    "Failed to write {log_entry:?} to the asset processor log. This is not recoverable. {error}"
)]
pub(crate) struct WriteLogError {
    pub(crate) log_entry: LogEntry,
    pub(crate) error: KairosError,
}

/// An error that occurs when validating the [`ProcessorTransactionLog`] fails.
#[derive(Error, Debug)]
pub enum ValidateLogError {
    /// An error that could not be recovered from. All assets will be reprocessed.
    #[error("Encountered an unrecoverable error. All assets will be reprocessed.")]
    UnrecoverableError,
    /// A [`ReadLogError`], already boxed into a [`KairosError`].
    #[error("Failed to read log entries: {0}")]
    ReadLogError(KairosError),
    /// Duplicated process asset transactions occurred.
    #[error("Encountered a duplicate process asset transaction: {0:?}")]
    EntryErrors(Vec<LogEntryError>),
}

/// An error that occurs when validating individual [`ProcessorTransactionLog`] entries.
#[derive(Error, Debug)]
pub enum LogEntryError {
    /// A duplicate process asset transaction occurred for the given asset path.
    #[error("Encountered a duplicate process asset transaction: {0}")]
    DuplicateTransaction(AssetPath<'static>),
    /// A transaction was ended that never started for the given asset path.
    #[error("A transaction was ended that never started {0}")]
    EndedMissingTransaction(AssetPath<'static>),
    /// An asset started processing but never finished at the given asset path.
    #[error("An asset started processing but never finished: {0}")]
    UnfinishedTransaction(AssetPath<'static>),
}

/// An error when attempting to set the transaction log factory.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum SetTransactionLogFactoryError {
    /// The log is already in use, so setting the factory does nothing.
    #[error("Transaction log is already in use so setting the factory does nothing")]
    AlreadyInUse,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        FileTransactionLogFactory, LogEntry, LogEntryError, ProcessorTransactionLog,
        ProcessorTransactionLogFactory, ReadLogError, ValidateLogError, validate_transaction_log,
    };
    use crate::path::AssetPath;
    use kairos_ecs::error::KairosError;
    use kairos_tasks::BoxedFuture;

    /// A [`ProcessorTransactionLogFactory`] backed by an in-memory entry list.
    #[derive(Clone, Default)]
    struct TestLogFactory {
        entries: Arc<Mutex<Vec<LogEntry>>>,
        /// Set when `read` should fail, to exercise the read-error path.
        read_error: Arc<Mutex<Option<String>>>,
    }

    impl ProcessorTransactionLogFactory for TestLogFactory {
        fn read(&self) -> BoxedFuture<'_, Result<Vec<LogEntry>, KairosError>> {
            let entries = self.entries.lock().unwrap().clone();
            let error = self.read_error.lock().unwrap().clone();
            Box::pin(async move {
                if let Some(message) = error {
                    return Err(ReadLogError::InvalidLine(message).into());
                }
                Ok(entries)
            })
        }

        fn create_new_log(
            &self,
        ) -> BoxedFuture<'_, Result<Box<dyn ProcessorTransactionLog>, KairosError>> {
            Box::pin(async move { Ok(Box::new(NoopLog) as Box<dyn ProcessorTransactionLog>) })
        }
    }

    struct NoopLog;

    impl ProcessorTransactionLog for NoopLog {
        fn begin_processing<'a>(
            &'a mut self,
            _asset: &'a AssetPath<'_>,
        ) -> BoxedFuture<'a, Result<(), KairosError>> {
            Box::pin(async move { Ok(()) })
        }

        fn end_processing<'a>(
            &'a mut self,
            _asset: &'a AssetPath<'_>,
        ) -> BoxedFuture<'a, Result<(), KairosError>> {
            Box::pin(async move { Ok(()) })
        }

        fn unrecoverable(&mut self) -> BoxedFuture<'_, Result<(), KairosError>> {
            Box::pin(async move { Ok(()) })
        }
    }

    fn factory(entries: Vec<LogEntry>) -> TestLogFactory {
        TestLogFactory {
            entries: Arc::new(Mutex::new(entries)),
            read_error: Arc::new(Mutex::new(None)),
        }
    }

    fn validate(entries: Vec<LogEntry>) -> Result<(), ValidateLogError> {
        futures_lite::future::block_on(validate_transaction_log(&factory(entries)))
    }

    #[test]
    fn a_complete_transaction_validates() {
        let asset = AssetPath::from("asset.bin");
        validate(vec![
            LogEntry::BeginProcessing(asset.clone()),
            LogEntry::EndProcessing(asset),
        ])
        .expect("a balanced log is valid");
    }

    #[test]
    fn an_unfinished_transaction_is_reported() {
        let asset = AssetPath::from("asset.bin");
        let error = validate(vec![LogEntry::BeginProcessing(asset.clone())])
            .expect_err("a begin with no end is unfinished");

        let ValidateLogError::EntryErrors(errors) = error else {
            panic!("expected entry errors");
        };
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            &errors[0],
            LogEntryError::UnfinishedTransaction(path) if *path == asset
        ));
    }

    #[test]
    fn a_duplicate_begin_is_reported() {
        let asset = AssetPath::from("asset.bin");
        let error = validate(vec![
            LogEntry::BeginProcessing(asset.clone()),
            LogEntry::BeginProcessing(asset.clone()),
        ])
        .expect_err("a duplicate begin is invalid");
        let ValidateLogError::EntryErrors(errors) = error else {
            panic!("expected entry errors");
        };
        assert!(matches!(
            &errors[0],
            LogEntryError::DuplicateTransaction(path) if *path == asset
        ));
    }

    #[test]
    fn an_end_without_a_begin_is_reported() {
        let asset = AssetPath::from("asset.bin");
        let error = validate(vec![LogEntry::EndProcessing(asset.clone())])
            .expect_err("an end with no begin is invalid");
        let ValidateLogError::EntryErrors(errors) = error else {
            panic!("expected entry errors");
        };
        assert!(matches!(
            &errors[0],
            LogEntryError::EndedMissingTransaction(path) if *path == asset
        ));
    }

    #[test]
    fn an_unrecoverable_entry_invalidates_everything() {
        let error = validate(vec![LogEntry::UnrecoverableError])
            .expect_err("an unrecoverable entry invalidates the log");
        assert!(matches!(error, ValidateLogError::UnrecoverableError));
    }

    #[test]
    fn a_read_failure_is_reported() {
        let factory = TestLogFactory {
            entries: Arc::new(Mutex::new(Vec::new())),
            read_error: Arc::new(Mutex::new(Some("not a valid line".to_string()))),
        };
        let error = futures_lite::future::block_on(validate_transaction_log(&factory))
            .expect_err("a read failure invalidates the log");
        assert!(matches!(error, ValidateLogError::ReadLogError(_)));
    }

    #[test]
    fn file_log_round_trips_and_detects_an_unfinished_transaction() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let dir = std::env::temp_dir().join(format!(
            "kairos_asset_log_test_{}_{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
        ));
        let factory = FileTransactionLogFactory {
            file_path: dir.join("log"),
        };

        let a = AssetPath::from("a.bin");
        let b = AssetPath::from("b.bin");

        futures_lite::future::block_on(async {
            let mut log = factory.create_new_log().await.unwrap();
            log.begin_processing(&a).await.unwrap();
            log.end_processing(&a).await.unwrap();
            log.begin_processing(&b).await.unwrap();
            drop(log);

            let entries = factory.read().await.unwrap();
            assert_eq!(
                entries,
                vec![
                    LogEntry::BeginProcessing(a.clone()),
                    LogEntry::EndProcessing(a.clone()),
                    LogEntry::BeginProcessing(b.clone()),
                ]
            );

            // `b` began but never ended, so validation flags it for recovery.
            let error = validate_transaction_log(&factory).await.unwrap_err();
            let ValidateLogError::EntryErrors(errors) = error else {
                panic!("expected entry errors");
            };
            assert!(matches!(
                &errors[0],
                LogEntryError::UnfinishedTransaction(path) if *path == b
            ));

            // Starting a new log clears the previous entries.
            factory.create_new_log().await.unwrap();
            assert!(factory.read().await.unwrap().is_empty());
        });

        let _ = std::fs::remove_dir_all(&dir);
    }
}
