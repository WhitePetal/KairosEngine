use std::sync::Arc;

use crate::{Record, sinks};

/// A destination for log records.
///
/// Sinks run on the emitting thread, so they must be cheap and thread-safe,
/// and must not log themselves. Every attached sink sees every record that
/// passes the process-level filter.
pub trait Sink: Send + Sync + 'static {
    /// Writes one record.
    fn write(&self, record: &Record);

    /// Flushes buffered output. Called by [`crate::flush`] and when the
    /// process [`crate::Guard`] drops.
    fn flush(&self) {}
}

/// Handle that keeps a sink attached; dropping it detaches the sink.
#[derive(Debug)]
pub struct SinkHandle {
    id: u64,
}

impl SinkHandle {
    pub(crate) fn new(id: u64) -> Self {
        Self { id }
    }
}

impl Drop for SinkHandle {
    fn drop(&mut self) {
        sinks::detach(self.id);
    }
}

/// Attaches `sink` to the process logger.
///
/// May be called before [`crate::init`]; the sink then starts receiving
/// records as soon as the logger is installed. The returned handle detaches
/// the sink when dropped.
pub fn attach(sink: Arc<dyn Sink>) -> SinkHandle {
    SinkHandle::new(sinks::attach(sink))
}
