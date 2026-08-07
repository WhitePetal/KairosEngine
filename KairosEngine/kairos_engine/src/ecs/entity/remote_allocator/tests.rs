use super::*;

/// Ensure the total capacity of [`OwnedBuffer`] is `u32::MAX + 1`.
#[test]
fn chunk_capacity_sums() {
    let total: u64 = (0..FreeBuffer::NUM_CHUNKS)
        .map(FreeBuffer::capacity_of_chunk)
        .map(|x| x as u64)
        .sum();
    // The last 2 won't be used, but that's ok.
    // Keeping them powers of 2 makes things faster.
    let expected = u32::MAX as u64 + 1;
    assert_eq!(total, expected);
}

/// Ensure [`OwnedBuffer`] can be properly indexed
#[test]
fn chunk_indexing() {
    let to_test = vec![
        (0, (0, 0, 512)), // index 0 cap = 512
        (1, (0, 1, 512)),
        (256, (0, 256, 512)),
        (511, (0, 511, 512)),
        (512, (1, 0, 512)), // index 1 cap = 512
        (1023, (1, 511, 512)),
        (1024, (2, 0, 1024)), // index 2 cap = 1024
        (1025, (2, 1, 1024)),
        (2047, (2, 1023, 1024)),
        (2048, (3, 0, 2048)), // index 3 cap = 2048
        (4095, (3, 2047, 2048)),
        (4096, (4, 0, 4096)), // index 3 cap = 4096
    ];

    for (input, output) in to_test {
        assert_eq!(FreeBuffer::index_info(input), output);
    }
}

#[test]
fn buffer_len_encoding() {
    let len = FreeCount::new_zero_len();
    assert_eq!(len.state(Ordering::Relaxed).length(), 0);
    assert_eq!(len.pop_for_state(200, Ordering::Relaxed).length(), 0);
    len.set_state_risky(
        FreeCountState::new_zero_len().with_length(5),
        Ordering::Relaxed,
    );
    assert_eq!(len.pop_for_state(2, Ordering::Relaxed).length(), 5);
    assert_eq!(len.pop_for_state(2, Ordering::Relaxed).length(), 3);
    assert_eq!(len.pop_for_state(2, Ordering::Relaxed).length(), 1);
    assert_eq!(len.pop_for_state(2, Ordering::Relaxed).length(), 0);
}

#[test]
fn uniqueness() {
    let mut entities = Vec::with_capacity(2000);
    let mut allocator = Allocator::new();
    entities.extend(allocator.alloc_many(1000));

    let pre_len = entities.len();
    entities.dedup();
    assert_eq!(pre_len, entities.len());

    for e in entities.drain(..) {
        allocator.free(e);
    }

    entities.extend(allocator.alloc_many(500));
    for _ in 0..1000 {
        entities.push(allocator.alloc());
    }
    entities.extend(allocator.alloc_many(500));

    let pre_len = entities.len();
    entities.dedup();
    assert_eq!(pre_len, entities.len());
}

/// Bevy's allocator doesn't make guarantees about what order entities will be allocated in.
/// This test just exists to make sure allocations don't step on each other's toes.
#[test]
fn allocation_order_correctness() {
    let mut allocator = Allocator::new();
    let e0 = allocator.alloc();
    let e1 = allocator.alloc();
    let e2 = allocator.alloc();
    let e3 = allocator.alloc();
    allocator.free(e0);
    allocator.free(e1);
    allocator.free(e2);
    allocator.free(e3);
    allocator.flush_freed();

    let r0 = allocator.alloc();
    let mut many = allocator.alloc_many(2);
    let r1 = many.next().unwrap();
    let r2 = many.next().unwrap();
    assert!(many.next().is_none());
    drop(many);
    let r3 = allocator.alloc();

    assert_eq!(r0, e3);
    assert_eq!(r1, e1);
    assert_eq!(r2, e2);
    assert_eq!(r3, e0);
}
