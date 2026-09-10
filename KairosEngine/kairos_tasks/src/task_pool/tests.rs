use std::{
    sync::{
        Arc, Barrier,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    thread,
};

use crate::task_pool::{TaskPool, TaskPoolBuilder};

#[test]
fn test_spawn() {
    let pool = TaskPool::new();

    let foo = Box::new(42);
    let foo = &*foo;

    let count = Arc::new(AtomicI32::new(0));

    let outputs = pool.scope(|scope| {
        for _ in 0..100 {
            let count_clone = count.clone();
            scope.spawn(async move {
                if *foo != 42 {
                    panic!("not 42!?!?")
                } else {
                    count_clone.fetch_add(1, Ordering::Relaxed);
                    *foo
                }
            });
        }
    });

    for output in &outputs {
        assert_eq!(*output, 42);
    }

    assert_eq!(outputs.len(), 100);
    assert_eq!(count.load(Ordering::Relaxed), 100);
}

#[test]
fn test_thread_callbacks() {
    let counter = Arc::new(AtomicI32::new(0));
    let start_counter = counter.clone();
    {
        let barrier = Arc::new(Barrier::new(11));
        let last_barrier = barrier.clone();
        // Build and immediately drop to terminate
        let _pool = TaskPoolBuilder::new()
            .num_threads(10)
            .on_thread_spawn(move || {
                start_counter.fetch_add(1, Ordering::Relaxed);
                barrier.clone().wait();
            })
            .build();
        last_barrier.wait();
        assert_eq!(10, counter.load(Ordering::Relaxed));
    }
    assert_eq!(10, counter.load(Ordering::Relaxed));
    let end_counter = counter.clone();
    {
        let _pool = TaskPoolBuilder::new()
            .num_threads(20)
            .on_thread_destroy(move || {
                end_counter.fetch_sub(1, Ordering::Relaxed);
            })
            .build();
        assert_eq!(10, counter.load(Ordering::Relaxed));
    }
    assert_eq!(-10, counter.load(Ordering::Relaxed));
    let start_counter = counter.clone();
    let end_counter = counter.clone();
    {
        let barrier = Arc::new(Barrier::new(6));
        let last_barrier = barrier.clone();
        let _pool = TaskPoolBuilder::new()
            .num_threads(5)
            .on_thread_spawn(move || {
                start_counter.fetch_add(1, Ordering::Relaxed);
                barrier.wait();
            })
            .on_thread_destroy(move || {
                end_counter.fetch_sub(1, Ordering::Relaxed);
            })
            .build();
        last_barrier.wait();
        assert_eq!(-5, counter.load(Ordering::Relaxed));
    }
    assert_eq!(-10, counter.load(Ordering::Relaxed));
}

#[test]
fn test_mixed_spawn_on_scope_and_spawn() {
    let pool = TaskPool::new();

    let foo = Box::new(42);
    let foo = &*foo;

    let local_count = Arc::new(AtomicI32::new(0));
    let non_local_count = Arc::new(AtomicI32::new(0));

    let outputs = pool.scope(|scope| {
        for i in 0..100 {
            if i % 2 == 0 {
                let count_clone = non_local_count.clone();
                scope.spawn(async move {
                    if *foo != 42 {
                        panic!("not 42!?!?")
                    } else {
                        count_clone.fetch_add(1, Ordering::Relaxed);
                        *foo
                    }
                });
            } else {
                let count_clone = local_count.clone();
                scope.spawn_on_scope(async move {
                    if *foo != 42 {
                        panic!("not 42!?!?")
                    } else {
                        count_clone.fetch_add(1, Ordering::Relaxed);
                        *foo
                    }
                });
            }
        }
    });

    for output in &outputs {
        assert_eq!(*output, 42);
    }

    assert_eq!(outputs.len(), 100);
    assert_eq!(local_count.load(Ordering::Relaxed), 50);
    assert_eq!(non_local_count.load(Ordering::Relaxed), 50);
}

#[test]
fn test_thread_locality() {
    let pool = Arc::new(TaskPool::new());
    let count = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(101));
    let thread_check_failed = Arc::new(AtomicBool::new(false));

    for _ in 0..100 {
        let inner_barrier = barrier.clone();
        let count_clone = count.clone();
        let inner_pool = pool.clone();
        let inner_thread_check_failed = thread_check_failed.clone();
        thread::spawn(move || {
            inner_pool.scope(|scope| {
                let inner_count_clone = count_clone.clone();
                scope.spawn(async move {
                    inner_count_clone.fetch_add(1, Ordering::Release);
                });
                let spawner = thread::current().id();
                let inner_count_clone = count_clone.clone();
                scope.spawn_on_scope(async move {
                    inner_count_clone.fetch_add(1, Ordering::Release);
                    if thread::current().id() != spawner {
                        // NOTE: This check is using an atomic rather than simply panicking the
                        // thread to avoid deadlocking the barrier on failure
                        inner_thread_check_failed.store(true, Ordering::Release);
                    }
                });
            });
            inner_barrier.wait();
        });
    }
    barrier.wait();
    assert!(!thread_check_failed.load(Ordering::Acquire));
    assert_eq!(count.load(Ordering::Acquire), 200);
}

#[test]
fn test_nested_spawn() {
    let pool = TaskPool::new();

    let foo = Box::new(42);
    let foo = &*foo;

    let count = Arc::new(AtomicI32::new(0));

    let outputs: Vec<i32> = pool.scope(|scope| {
        for _ in 0..10 {
            let count_clone = count.clone();
            scope.spawn(async move {
                for _ in 0..10 {
                    let count_clone_clone = count_clone.clone();
                    scope.spawn(async move {
                        if *foo != 42 {
                            panic!("not 42!?!?")
                        } else {
                            count_clone_clone.fetch_add(1, Ordering::Relaxed);
                            *foo
                        }
                    });
                }
                *foo
            });
        }
    });

    for output in &outputs {
        assert_eq!(*output, 42);
    }

    // the inner loop runs 100 times and the outer one runs 10. 100 + 10
    assert_eq!(outputs.len(), 110);
    assert_eq!(count.load(Ordering::Relaxed), 100);
}

#[test]
fn test_nested_locality() {
    let pool = Arc::new(TaskPool::new());
    let count = Arc::new(AtomicI32::new(0));
    let barrier = Arc::new(Barrier::new(101));
    let thread_check_failed = Arc::new(AtomicBool::new(false));

    for _ in 0..100 {
        let inner_barrier = barrier.clone();
        let count_clone = count.clone();
        let inner_pool = pool.clone();
        let inner_thread_check_failed = thread_check_failed.clone();
        thread::spawn(move || {
            inner_pool.scope(|scope| {
                let spawner = thread::current().id();
                let inner_count_clone = count_clone.clone();
                scope.spawn(async move {
                    inner_count_clone.fetch_add(1, Ordering::Release);

                    // spawning on the scope from another thread runs the futures on the scope's thread
                    scope.spawn_on_scope(async move {
                        inner_count_clone.fetch_add(1, Ordering::Release);
                        if thread::current().id() != spawner {
                            // NOTE: This check is using an atomic rather than simply panicking the
                            // thread to avoid deadlocking the barrier on failure
                            inner_thread_check_failed.store(true, Ordering::Release);
                        }
                    });
                });
            });
            inner_barrier.wait();
        });
    }
    barrier.wait();
    assert!(!thread_check_failed.load(Ordering::Acquire));
    assert_eq!(count.load(Ordering::Acquire), 200);
}

// This test will often freeze on other executors.
#[test]
fn test_nested_scopes() {
    let pool = TaskPool::new();
    let count = Arc::new(AtomicI32::new(0));

    pool.scope(|scope| {
        scope.spawn(async {
            pool.scope(|scope| {
                scope.spawn(async {
                    count.fetch_add(1, Ordering::Relaxed);
                });
            });
        });
    });

    assert_eq!(count.load(Ordering::Acquire), 1);
}
