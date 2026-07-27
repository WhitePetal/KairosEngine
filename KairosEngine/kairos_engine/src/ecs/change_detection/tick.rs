use std::ops::{Add, Sub};

#[cfg(test)]
mod test;

/// 一个包装了 `u32` 的变更检测 tick，使用 wrapping 算术。
///
/// 每个 `System` 持有自己的 `last_run` tick，通过
/// [`Tick::is_newer_than`] 判断组件是否在系统上次运行后被修改过。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub u32);

impl Tick {
    /// 最大 tick 值。
    pub const MAX: Tick = Tick(u32::MAX);
    /// 最小 tick 值。
    pub const MIN: Tick = Tick(0u32);

    /// 变更检测可追踪的最大 tick 跨度。
    ///
    /// 当两次 tick 之差超过此值时，认为变更"太老"而被忽略，这可以避免
    /// wrapping 后的误报。
    pub const MAX_CHANGE_AGE: u32 = u32::MAX / 2;

    /// 创建一个新的 `Tick`。
    #[inline]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// 返回 `self` 到 `other` 的 wrapping 距离（`other - self`）。
    ///
    /// 相当于 `other.0.wrapping_sub(self.0)`，但返回 [`Tick`]。
    #[inline]
    pub fn relative_to(self, other: Tick) -> Tick {
        Tick(other.0.wrapping_sub(self.0))
    }

    /// 判断组件变更 tick `self` 是否比系统上次运行更新。
    ///
    /// `last_run` — 系统上次运行时的 tick。
    /// `this_run` — 系统本次运行时的 tick。
    ///
    /// 返回 `true` 表示组件在系统上次运行之后被修改过。
    #[inline]
    pub fn is_newer_than(self, last_run: Tick, this_run: Tick) -> bool {
        let ticks_since_change = self.relative_to(this_run).0.min(Self::MAX_CHANGE_AGE);
        let ticks_since_system = last_run.relative_to(this_run).0.min(Self::MAX_CHANGE_AGE);
        ticks_since_change < ticks_since_system
    }
}

impl Add<u32> for Tick {
    type Output = Tick;

    #[inline]
    fn add(self, rhs: u32) -> Tick {
        Tick(self.0.wrapping_add(rhs))
    }
}

impl Sub<u32> for Tick {
    type Output = Tick;

    #[inline]
    fn sub(self, rhs: u32) -> Tick {
        Tick(self.0.wrapping_sub(rhs))
    }
}

impl From<Tick> for u32 {
    #[inline]
    fn from(tick: Tick) -> u32 {
        tick.0
    }
}

impl From<u32> for Tick {
    #[inline]
    fn from(value: u32) -> Tick {
        Tick(value)
    }
}

/// 每个组件实例存储两个 tick：`added`（插入 tick）和 `changed`（最近修改 tick）。
///
/// - **插入**（`spawn`/`insert`）：同时设置 `added` 和 `changed` 为当前 tick。
/// - **修改**（通过 `&mut T`）：只更新 `changed`，保留 `added`。
///
/// 这样 `Added<T>` 和 `Changed<T>` 过滤器就能正确区分"新增"和"修改"。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ComponentTicks {
    /// 组件被插入（`spawn`/`insert`）时的 tick。
    pub added: Tick,
    /// 组件最近一次被修改时的 tick。
    pub changed: Tick,
}

impl ComponentTicks {
    /// 创建一个新的 `ComponentTicks`，`added` 和 `changed` 都设为给定 tick。
    ///
    /// 用于插入操作（`spawn`/`insert`）。
    #[inline]
    pub const fn new(change_tick: Tick) -> Self {
        Self {
            added: change_tick,
            changed: change_tick,
        }
    }

    /// 更新 `changed` 为当前 tick，保留 `added` 不变。
    ///
    /// 在通过 `&mut T` 访问组件时自动调用。
    #[inline]
    pub fn set_changed(&mut self, change_tick: Tick) {
        self.changed = change_tick;
    }

    /// 判断组件是否是在系统上次运行后新插入的。
    #[inline]
    pub fn is_added(self, last_run: Tick, this_run: Tick) -> bool {
        self.added.is_newer_than(last_run, this_run)
    }

    /// 判断组件是否是在系统上次运行后被修改过（包括插入）。
    #[inline]
    pub fn is_changed(self, last_run: Tick, this_run: Tick) -> bool {
        self.changed.is_newer_than(last_run, this_run)
    }
}
