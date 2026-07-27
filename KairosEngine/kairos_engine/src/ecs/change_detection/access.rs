use std::ops::{Deref, DerefMut};

use super::{ComponentTicks, Tick};

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// 变更检测的只读接口。
///
/// 提供查询组件变更状态的方法，适用于 [`Ref`] 和 [`Mut`]。
pub trait DetectChanges {
    /// 被包装的内部类型。
    type Inner: ?Sized;

    /// 如果组件是在系统上次运行后 **被修改过**（包括插入时），返回 `true`。
    fn is_changed(&self) -> bool;

    /// 如果组件是在系统上次运行后 **被插入** 的，返回 `true`。
    fn is_added(&self) -> bool;

    /// 组件最近一次被修改时的 tick。
    fn last_changed(&self) -> Tick;
}

/// 变更检测的可变接口。
///
/// 提供手动标记变更、绕过变更检测等方法。适用于 [`Mut`]。
pub trait DetectChangesMut: DetectChanges {
    /// 手动将组件标记为"已修改"（设置 `changed` 为 `this_run`）。
    fn set_changed(&mut self);

    /// 手动将组件标记为"已插入"（同时设置 `added` 和 `changed` 为 `this_run`）。
    fn set_added(&mut self);

    /// 返回 `&mut Self::Inner`，**不触发** 变更检测标记。
    ///
    /// 当你修改组件语义上不属于"变更"时（如修复一个缓存值），
    /// 可以使用此方法避免触发 `Changed<T>` 检测。
    fn bypass_change_detection(&mut self) -> &mut Self::Inner;
}

// ---------------------------------------------------------------------------
// Ref — 只读 wrapper，携带 tick 信息
// ---------------------------------------------------------------------------

/// 携带变更检测 tick 信息的只读组件引用。
///
/// 通过 [`Deref`] 提供对 `T` 的不可变访问，同时提供 [`is_changed`](Ref::is_changed)
/// 和 [`is_added`](Ref::is_added) 方法查询组件的变更状态。
///
/// `Ref` 实现了 [`Copy`] 和 [`Clone`]，可以自由共享。
pub struct Ref<'w, T: ?Sized> {
    /// 指向组件数据的不可变引用
    pub(crate) value: &'w T,
    /// 该组件的 tick 信息（可选，为 null 时表示无 tick 跟踪）
    pub(crate) ticks: Option<&'w ComponentTicks>,
    /// 系统上次运行时的 tick
    pub(crate) last_run: Tick,
    /// 系统本次运行时的 tick
    pub(crate) this_run: Tick,
}

// 手动实现 Clone + Copy：因为所有字段都是 Copy 类型（引用、Tick），所以不要求 `T: Copy`。
impl<T: ?Sized> Clone for Ref<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized> Copy for Ref<'_, T> {}

// SAFETY: Ref 包含 `&'w T` 和 `Option<&'w ComponentTicks>`，都是共享引用。
// 只要 `T: Sync`，Ref 就可以是 `Send + Sync`。
unsafe impl<T: ?Sized + Sync> Send for Ref<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for Ref<'_, T> {}

impl<T: ?Sized> Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> DetectChanges for Ref<'_, T> {
    type Inner = T;

    #[inline]
    fn is_changed(&self) -> bool {
        self.ticks
            .map(|t| t.is_changed(self.last_run, self.this_run))
            .unwrap_or(false)
    }

    #[inline]
    fn is_added(&self) -> bool {
        self.ticks
            .map(|t| t.is_added(self.last_run, self.this_run))
            .unwrap_or(false)
    }

    #[inline]
    fn last_changed(&self) -> Tick {
        self.ticks.map(|t| t.changed).unwrap_or(Tick::MIN)
    }
}

impl<T: ?Sized> Ref<'_, T> {
    /// 返回原始组件引用。
    #[inline]
    pub fn into_inner(self) -> &'static T {
        // SAFETY: 这是一个生命周期擦除操作。调用者必须保证返回的引用不会超出
        // 原始数据的生命周期。此方法主要用于需要在特定生命周期约束下工作的场景。
        unsafe { &*(self.value as *const T) }
    }
}

// ---------------------------------------------------------------------------
// Mut — 可变 wrapper，DerefMut 自动标记 changed
// ---------------------------------------------------------------------------

/// 携带变更检测 tick 信息的可变组件引用。
///
/// 通过 [`Deref`] 提供对 `T` 的不可变访问，通过 [`DerefMut`] 提供可变访问。
/// **每次通过 `DerefMut` 访问时，自动将组件的 `changed` tick 标记为 `this_run`**。
///
/// 如果需要修改组件但不希望触发变更检测，使用
/// [`bypass_change_detection`](Mut::bypass_change_detection)。
pub struct Mut<'w, T: ?Sized> {
    /// 指向组件数据的可变引用
    value: &'w mut T,
    /// 该组件的可变 tick 指针
    ticks: Option<&'w mut ComponentTicks>,
    /// 系统上次运行时的 tick
    last_run: Tick,
    /// 系统本次运行时的 tick
    this_run: Tick,
}

// SAFETY: Mut 包含 `&'w mut T` 和 `Option<&'w mut ComponentTicks>`，是唯一访问。
// 只要 `T: Send`，Mut 就可以是 Send。
unsafe impl<T: ?Sized + Send> Send for Mut<'_, T> {}
unsafe impl<T: ?Sized + Sync> Sync for Mut<'_, T> {}

impl<T: ?Sized> Deref for Mut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T: ?Sized> DerefMut for Mut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.ticks_changed();
        self.value
    }
}

impl<T: ?Sized> DetectChanges for Mut<'_, T> {
    type Inner = T;

    #[inline]
    fn is_changed(&self) -> bool {
        self.ticks
            .as_ref()
            .map(|t| t.is_changed(self.last_run, self.this_run))
            .unwrap_or(false)
    }

    #[inline]
    fn is_added(&self) -> bool {
        self.ticks
            .as_ref()
            .map(|t| t.is_added(self.last_run, self.this_run))
            .unwrap_or(false)
    }

    #[inline]
    fn last_changed(&self) -> Tick {
        self.ticks.as_ref().map(|t| t.changed).unwrap_or(Tick::MIN)
    }
}

impl<T: ?Sized> DetectChangesMut for Mut<'_, T> {
    #[inline]
    fn set_changed(&mut self) {
        if let Some(ref mut ticks) = self.ticks {
            ticks.set_changed(self.this_run);
        }
    }

    #[inline]
    fn set_added(&mut self) {
        if let Some(ref mut ticks) = self.ticks {
            ticks.added = self.this_run;
            ticks.changed = self.this_run;
        }
    }

    #[inline]
    fn bypass_change_detection(&mut self) -> &mut T {
        self.value
    }
}

impl<'w, T: ?Sized> Mut<'w, T> {
    /// 创建一个新的 `Mut`。
    #[inline]
    pub(crate) fn new(
        value: &'w mut T,
        ticks: Option<&'w mut ComponentTicks>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        Self {
            value,
            ticks,
            last_run,
            this_run,
        }
    }

    /// 返回原始可变组件引用（不触发变更检测）。
    #[inline]
    pub fn into_inner(self) -> &'w mut T {
        self.value
    }

    /// 标记 changed 的内部方法。
    #[inline]
    fn ticks_changed(&mut self) {
        if let Some(ref mut ticks) = self.ticks {
            ticks.set_changed(self.this_run);
        }
    }
}

impl<'w, T: Sized> Mut<'w, T> {
    /// 仅当新值与当前值 **不同** 时才写入并标记为已变更。
    ///
    /// 如果值相同，则不做任何写入操作。
    #[inline]
    pub fn set_if_neq(&mut self, value: T)
    where
        T: PartialEq,
    {
        if *self.value != value {
            *self.value = value;
            self.ticks_changed();
        }
    }
}
