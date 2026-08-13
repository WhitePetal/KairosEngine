//! Types for controlling batching behavior during parallel processing.

use std::ops::Range;

/// Dictates how a parallel operation chunks up large quantities
/// during iteration.
///
/// A parallel message reader will chunk up the unread messages into
/// batches of at most a certain size, which are then distributed across
/// the available threads.
///
/// By default, this batch size is automatically determined by dividing
/// the number of unread messages by the number of threads (rounded up).
/// This attempts to minimize the overhead of scheduling work onto
/// multiple threads, but assumes each message takes roughly the same
/// amount of work to process, which may not hold true in every workload.
///
/// See [`MessageParIter::batching_strategy`](crate::ecs::message::MessageParIter::batching_strategy)
/// for more information.
#[derive(Clone, Debug)]
pub struct BatchingStrategy {
    /// The upper and lower limits for a batch of items.
    ///
    /// Setting the bounds to the same value will result in a fixed
    /// batch size.
    ///
    /// Defaults to `[1, usize::MAX]`.
    pub batch_size_limits: Range<usize>,
    /// The number of batches to assign to each thread.
    /// Increasing this value will decrease the batch size, which may
    /// increase the scheduling overhead for the iteration.
    ///
    /// Defaults to 1.
    pub batches_per_thread: usize,
}

impl Default for BatchingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl BatchingStrategy {
    /// Creates a new unconstrained default batching strategy.
    pub const fn new() -> Self {
        Self {
            batch_size_limits: 1..usize::MAX,
            batches_per_thread: 1,
        }
    }

    /// Declares a batching strategy with a fixed batch size.
    pub const fn fixed(batch_size: usize) -> Self {
        Self {
            batch_size_limits: batch_size..batch_size,
            batches_per_thread: 1,
        }
    }

    /// Configures the minimum allowed batch size of this instance.
    pub const fn min_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size_limits.start = batch_size;
        self
    }

    /// Configures the maximum allowed batch size of this instance.
    pub const fn max_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size_limits.end = batch_size;
        self
    }

    /// Configures the number of batches to assign to each thread for this instance.
    pub fn batches_per_thread(mut self, batches_per_thread: usize) -> Self {
        assert!(
            batches_per_thread > 0,
            "The number of batches per thread must be non-zero."
        );
        self.batches_per_thread = batches_per_thread;
        self
    }

    /// Calculate the batch size according to the given thread count and max item count.
    /// The count is provided as a closure so that it can be calculated only if needed.
    ///
    /// # Panics
    ///
    /// Panics if `thread_count` is 0.
    #[inline]
    pub fn calc_batch_size(&self, max_items: impl FnOnce() -> usize, thread_count: usize) -> usize {
        if self.batch_size_limits.is_empty() {
            return self.batch_size_limits.start;
        }
        assert!(
            thread_count > 0,
            "Attempted to run parallel iteration with an empty thread pool"
        );
        let batches = thread_count * self.batches_per_thread;
        // Round up to the nearest batch size.
        let batch_size = max_items().div_ceil(batches);
        batch_size.clamp(self.batch_size_limits.start, self.batch_size_limits.end)
    }
}
