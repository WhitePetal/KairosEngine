//! Conditionally-required [`Send`] bounds, ported from `bevy_tasks`.
//!
//! Futures on native targets must be [`Send`] so they can be moved between
//! worker threads. On targets where that isn't possible (for example `wasm32`),
//! futures are routinely `!Send`, so the same bound must be dropped.
//! [`ConditionalSend`] and [`ConditionalSendFuture`] express "`Send` where the
//! target requires it": on every target except `wasm32` they are equivalent to
//! [`Send`], while on `wasm32` they impose no bound at all.
//!
//! This lets a single trait method signature, such as `AssetLoader::load`,
//! produce a future that is `Send` on native and `!Send`-friendly on the web.

/// Marks a type as [`Send`] on every target except `wasm32`.
///
/// This is equivalent to [`Send`] here, and is implemented for every `Send`
/// type. Use it instead of a bare [`Send`] bound so the same signature also
/// compiles where futures aren't `Send`.
#[cfg(not(target_arch = "wasm32"))]
pub trait ConditionalSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> ConditionalSend for T {}

/// A vacuous counterpart to [`ConditionalSend`] for targets where futures
/// aren't [`Send`].
///
/// This is implemented for every type, so it never restricts a bound.
#[cfg(target_arch = "wasm32")]
pub trait ConditionalSend {}

#[cfg(target_arch = "wasm32")]
impl<T> ConditionalSend for T {}

/// A [`Future`] with a conditionally-required [`Send`] bound.
///
/// On every target except `wasm32` this is equivalent to `Future + Send`; on
/// `wasm32` it is equivalent to [`Future`].
pub trait ConditionalSendFuture: Future + ConditionalSend {}

impl<T: Future + ConditionalSend> ConditionalSendFuture for T {}

#[cfg(test)]
mod tests {
    use super::{ConditionalSend, ConditionalSendFuture};

    #[test]
    fn send_types_are_conditional_send() {
        fn assert_conditional_send<T: ConditionalSend>() {}

        assert_conditional_send::<u32>();
        assert_conditional_send::<String>();
    }

    #[test]
    fn send_future_is_conditional_send_future_and_runs() {
        fn run<F: ConditionalSendFuture<Output = u32>>(future: F) -> u32 {
            futures_lite::future::block_on(future)
        }

        async fn answer() -> u32 {
            42
        }

        assert_eq!(run(answer()), 42);
    }

    /// On every target except `wasm32`, `ConditionalSend` must imply `Send`, so
    /// a future known only as `ConditionalSendFuture` can be sent to another
    /// thread.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn multithreaded_conditional_send_implies_send() {
        // Compiles only because `ConditionalSend: Send` on this target.
        fn conditional_send_implies_send<T: ConditionalSend + 'static>() {
            fn requires_send<T: Send>() {}
            requires_send::<T>();
        }
        conditional_send_implies_send::<u32>();

        // Compiles only because `ConditionalSendFuture: Future + Send` here.
        fn spawn<F: ConditionalSendFuture<Output = u32> + 'static>(future: F) -> u32 {
            std::thread::spawn(move || futures_lite::future::block_on(future))
                .join()
                .unwrap()
        }

        async fn answer() -> u32 {
            42
        }
        assert_eq!(spawn(answer()), 42);
    }

    /// Where futures aren't `Send`, a `!Send` future must still satisfy
    /// `ConditionalSendFuture`.
    #[cfg(target_arch = "wasm32")]
    #[test]
    fn non_send_future_is_conditional_send_future() {
        use std::rc::Rc;

        // `Rc` is `!Send`, yet it must implement `ConditionalSend` here.
        fn assert_conditional_send<T: ConditionalSend>() {}
        assert_conditional_send::<Rc<u32>>();

        fn run<F: ConditionalSendFuture<Output = u32>>(future: F) -> u32 {
            futures_lite::future::block_on(future)
        }

        async fn answer(value: Rc<u32>) -> u32 {
            *value
        }

        assert_eq!(run(answer(Rc::new(42))), 42);
    }
}
