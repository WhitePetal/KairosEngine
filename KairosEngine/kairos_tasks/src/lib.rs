mod executor;
pub mod futures;
mod iter;
mod slice;
mod task_pool;
mod thread_executor;
mod usages;

pub use async_task::Task;
pub use iter::ParallelIterator;
pub use slice::{ParallelSlice, ParallelSliceMut};
pub use task_pool::{Scope, TaskPool, TaskPoolBuilder};
pub use thread_executor::{ThreadExecutor, ThreadExecutorTicker};

/// Gets the logical CPU core count available to the current process.
///
/// This is identical to `std::thread::available_parallelism`, except
/// it will return a default value of 1 if it internally errors out.
///
/// This will always return at least 1.
pub fn available_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZero::<usize>::get)
        .unwrap_or(1)
}
