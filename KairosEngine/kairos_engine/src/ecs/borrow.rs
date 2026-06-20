use std::sync::atomic::{AtomicUsize, Ordering};

/// AtomicBorrow 表示是否存在可变借用的 位掩码
const UNIQUE_BIT: usize = !(usize::MAX >> 1);
/// AtomicBorrow 的不可变借用计数器 位掩码
const COUNTER_MASK: usize = usize::MAX >> 1;

/// 一个用来做动态借用检查的原子整数
///
/// 最高位用于表示是否存在可变借用，其他位作为不可变借用计数器
///
/// 有4种可能的状态：
///  - `0b00000000...` 没有被可变借用，且当前没有被不可变借用
///  - `0b0_______...` 没有被可变借用，且当前被可变借用
///  - `0b10000000...` 被可变借用
///  - `0b1_______...` 被可变借用，且当前有其他线程在尝试进行可变借用
#[derive(Debug)]
pub struct AtomicBorrow(AtomicUsize);

impl AtomicBorrow {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    pub fn borrow(&self) -> bool {
        let prev_value = self.0.fetch_add(1, Ordering::Acquire);

        if prev_value & COUNTER_MASK == COUNTER_MASK {
            core::panic!("immutable borrow counter overflowed")
        }

        if prev_value & UNIQUE_BIT != 0 {
            self.0.fetch_sub(1, Ordering::Release);
            false
        } else {
            true
        }
    }

    pub fn borrow_mut(&self) -> bool {
        // 只有当前没有任何借用(0)时，才能进行可变借用
        // 并将自身设置为可变借用状态(UNIQUE_BIT)
        self.0
            .compare_exchange(0, UNIQUE_BIT, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    pub fn release(&self) {
        let value = self.0.fetch_sub(1, Ordering::Release);
        debug_assert!(value != 0, "unbalanced release");
        debug_assert!(value & UNIQUE_BIT == 0, "shared release of unique borrow");
    }

    pub fn release_mut(&self) {
        let value = self.0.fetch_and(!UNIQUE_BIT, Ordering::Release);
        debug_assert_ne!(value & UNIQUE_BIT, 0, "unique release of shared borrow");
    }
}
