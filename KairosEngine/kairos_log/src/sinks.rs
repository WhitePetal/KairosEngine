//! Concrete sinks plus the process-wide sink registry.

mod file;
mod terminal;

pub(crate) use file::FileSink;
pub(crate) use terminal::TerminalSink;

use std::sync::{
    Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
    atomic::{AtomicU64, Ordering},
};

use crate::{Record, Sink};

/// Sinks attached for the process lifetime, in attach order. `init` adds the
/// default sinks; `attach` adds extra ones and removes them with their handle.
static SINKS: RwLock<Vec<(u64, Arc<dyn Sink>)>> = RwLock::new(Vec::new());
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn sinks() -> RwLockReadGuard<'static, Vec<(u64, Arc<dyn Sink>)>> {
    SINKS.read().unwrap_or_else(PoisonError::into_inner)
}

fn sinks_mut() -> RwLockWriteGuard<'static, Vec<(u64, Arc<dyn Sink>)>> {
    SINKS.write().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn attach(sink: Arc<dyn Sink>) -> u64 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    sinks_mut().push((id, sink));
    id
}

pub(crate) fn detach(id: u64) {
    sinks_mut().retain(|(sink_id, _)| *sink_id != id);
}

pub(crate) fn write(record: &Record) {
    for (_, sink) in sinks().iter() {
        sink.write(record);
    }
}

pub(crate) fn flush() {
    for (_, sink) in sinks().iter() {
        sink.flush();
    }
}
