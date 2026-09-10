use std::{sync::Arc, thread};

use crate::thread_executor::ThreadExecutor;

#[test]
fn test_ticker() {
    let executor = Arc::new(ThreadExecutor::new());
    let ticker = executor.ticker();
    assert!(ticker.is_some());

    thread::scope(|s| {
        s.spawn(|| {
            let ticker = executor.ticker();
            assert!(ticker.is_none());
        });
    });
}
