use std::{cell::UnsafeCell, panic::Location};

use crate::{debug::MaybeLocation, ecs::change_detection::{ComponentTickCells, Tick}, ptr::{ThinSlicePtr, UnsafeCellDeref}};



/// Used by immutable query parameters (such as [`Ref`] and [`Res`])
/// to store immutable access to the [`Tick`]s of a single component or resource.
#[derive(Clone, Copy)]
pub(crate) struct ComponentTickRef<'w> {
    pub(crate) added: &'w Tick,
    pub(crate) changed: &'w Tick,
    pub(crate) changed_by: MaybeLocation<&'w &'static Location<'static>>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'w> ComponentTickRef<'w> {
    /// # Safety
    /// This should never alias the underlying ticks with a mutable one such as `ComponentTicksMut`.
    #[inline]
    pub(crate) unsafe fn from_tick_cells(
        cells: ComponentTickCells<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        Self {
            added: unsafe {
                cells.added.deref()
            },
            changed: unsafe {
                cells.changed.deref()
            },
            changed_by: unsafe {
                cells.changed_by.map(|changed_by| changed_by.deref())
            },
            last_run,
            this_run
        }
    }
}


/// Data type storing contiguously lying ticks.
///
/// Retrievable via [`ContiguousRef::split`] and probably only useful if you want to use the following
/// methods:
/// - [`ContiguousComponentTicksRef::is_changed_iter`],
/// - [`ContiguousComponentTicksRef::is_added_iter`]
#[derive(Clone)]
pub struct ContiguousComponentTicksRef<'w> {
    pub(crate) added: &'w [Tick],
    pub(crate) changed: &'w [Tick],
    pub(crate) changed_by: MaybeLocation<&'w [&'static Location<'static>]>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'w> ContiguousComponentTicksRef<'w> {
    /// # Safety
    /// - The caller must have permission for all given ticks to be read.
    /// - `len` must be the length of `added`, `changed` and `changed_by` (unless none) slices.
    pub(crate) unsafe fn from_slice_ptrs(
        added: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        changed: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        change_by: MaybeLocation<ThinSlicePtr<'w, UnsafeCell<&'static Location<'static>>>>,
        len: usize,
        this_run: Tick,
        last_run: Tick
    ) -> Self {
        Self {
            // SAFETY:
            // - The caller ensures that `len` is the length of the slice.
            // - The caller ensures we have permission to read the data.
            added: unsafe {
                added.cast().as_slice_unchecked(len)
            },
            // SAFETY: see above.
            changed: unsafe {
                changed.cast().as_slice_unchecked(len)
            },
            // SAFETY: see above.
            changed_by: change_by.map(|v| unsafe {
                v.cast().as_slice_unchecked(len)
            }),
            last_run,
            this_run
        }
    }

    /// Creates a new `ContiguousComponentTicksRef` using provided values or returns [`None`] if lengths of
    /// `added`, `changed` and `changed_by` do not match
    ///
    /// This is an advanced feature, `ContiguousComponentTicksRef`s are designed to be _created_ by
    /// engine-internal code and _consumed_ by end-user code.
    ///
    /// - `added` - [`Tick`]s that store the tick when the wrapped value was created.
    /// - `changed` - [`Tick`]s that store the last time the wrapped value was changed.
    /// - `last_run` - A [`Tick`], occurring before `this_run`, which is used
    ///   as a reference to determine whether the wrapped value is newly added or changed.
    /// - `this_run` - A [`Tick`] corresponding to the current point in time -- "now".
    /// - `caller` - [`Location`]s that store the location when the wrapper value was changed.
    pub fn new(
        added: &'w [Tick],
        changed: &'w [Tick],
        last_run: Tick,
        this_run: Tick,
        caller: MaybeLocation<&'w [&'static Location<'static>]>
    ) -> Option<Self> {
        let eq = added.len() == changed.len()
            && caller
                .map(|v| v.len() == added.len())
                .into_option()
                .unwrap_or(true);
        eq.then_some(Self {
            added,
            changed,
            changed_by: caller,
            last_run,
            this_run
        })
    }

    /// Returns added ticks' slice.
    pub fn added(&self) -> &'w [Tick] {
        self.added
    }

    /// Returns changed ticks' slice.
    pub fn changed(&self) -> &'w [Tick] {
        self.changed
    }

    /// Returns changed by locations' slice.
    pub fn changed_by(&self) -> MaybeLocation<&[&'static Location<'static>]> {
        self.changed_by.as_deref()
    }

    /// Returns the tick the system last ran.
    pub fn last_run(&self) -> Tick {
        self.last_run
    }

    /// Returns the tick of the current system's run.
    pub fn this_run(&self) -> Tick {
        self.this_run
    }

    /// Returns an iterator where the i-th item corresponds to whether the i-th component was
    /// marked as changed. If the value equals [`prim@true`], then the component was changed.
    ///
    /// # Example
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct A(pub i32);
    ///
    /// fn some_system(mut query: Query<Ref<A>>) {
    ///     for a in query.contiguous_iter().unwrap() {
    ///         let (a_values, a_ticks) = ContiguousRef::split(a);
    ///         for (value, is_changed) in a_values.iter().zip(a_ticks.is_changed_iter()) {
    ///             if is_changed {
    ///                 // do something
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub fn is_changed_iter(&self) -> impl Iterator<Item = bool> {
        self.changed
            .iter()
            .map(|v| v.is_newer_than(self.last_run, self.this_run))
    }

    /// Returns an iterator where the i-th item corresponds to whether the i-th component was
    /// marked as added. If the value equals [`prim@true`], then the component was added.
    ///
    /// # Example
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct A(pub i32);
    ///
    /// fn some_system(mut query: Query<Ref<A>>) {
    ///     for a in query.contiguous_iter().unwrap() {
    ///         let (a_values, a_ticks) = ContiguousRef::split(a);
    ///         for (value, is_added) in a_values.iter().zip(a_ticks.is_added_iter()) {
    ///             if is_added {
    ///                 // do something
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub fn is_added_iter(&self) -> impl Iterator<Item = bool> {
        self.added
            .iter()
            .map(|v| v.is_newer_than(self.last_run, self.this_run))
    }
}


/// Used by mutable query parameters (such as [`Mut`] and [`ResMut`])
/// to store mutable access to the [`Tick`]s of a single component or resource.
pub(crate) struct ComponentTicksMut<'w> {
    pub(crate) added: &'w mut Tick,
    pub(crate) changed: &'w mut Tick,
    pub(crate) changed_by: MaybeLocation<&'w mut &'static Location<'static>>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'w> ComponentTicksMut<'w> {
    /// # Safety
    /// This should never alias the underlying ticks. All access must be unique.
    #[inline]
    pub(crate) unsafe fn from_tick_cells(
        cells: ComponentTickCells<'w>,
        last_run: Tick,
        this_run: Tick
    ) -> Self {
        Self {
            added: unsafe {
                cells.added.deref_mut()
            },
            changed: unsafe {
                cells.changed.deref_mut()
            },
            changed_by: unsafe {
                cells.changed_by.map(|changed_by| changed_by.deref_mut())
            },
            last_run,
            this_run
        }
    }
}

impl<'w> From<ComponentTicksMut<'w>> for ComponentTickRef<'w> {
    fn from(ticks: ComponentTicksMut<'w>) -> Self {
        ComponentTickRef {
            added: ticks.added,
            changed: ticks.changed,
            changed_by: ticks.changed_by.map(|changed_by| &*changed_by),
            last_run: ticks.last_run,
            this_run: ticks.this_run
        }
    }
}

/// Data type storing contiguously lying ticks, which may be accessed to mutate.
///
/// Retrievable via [`ContiguousMut::split`] and probably only useful if you want to use the following
/// methods:
/// - [`ContiguousComponentTicksMut::is_changed_iter`],
/// - [`ContiguousComponentTicksMut::is_added_iter`]
pub struct ContiguousComponentTicksMut<'w> {
    pub(crate) added: &'w mut [Tick],
    pub(crate) changed: &'w mut [Tick],
    pub(crate) changed_by: MaybeLocation<&'w mut [&'static Location<'static>]>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'w> ContiguousComponentTicksMut<'w> {
    /// # Safety
    /// - The caller must have permission to use all given ticks to be mutated.
    /// - `len` must be the length of `added`, `changed` and `changed_by` (unless none) slices.
    pub(crate) unsafe fn from_slice_ptrs(
        added: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        changed: ThinSlicePtr<'w, UnsafeCell<Tick>>,
        changed_by: MaybeLocation<ThinSlicePtr<'w, UnsafeCell<&'static Location<'static>>>>,
        len: usize,
        this_run: Tick,
        last_run: Tick
    ) -> Self {
        Self {
            // SAFETY:
            // - The caller ensures that `len` is the length of the slice.
            // - The caller ensures we have permission to mutate the data.
            added: unsafe {
                added.as_mut_slice_unchecked(len)
            },
            // SAFETY: see above.
            changed: unsafe {
                changed.as_mut_slice_unchecked(len)
            },
            // SAFETY: see above.
            changed_by: changed_by.map(|v| unsafe {
                v.as_mut_slice_unchecked(len)
            }),
            last_run,
            this_run
        }
    }

    /// Creates a new `ContiguousComponentTicksMut` using provided values or returns [`None`] if lengths of
    /// `added`, `changed` and `changed_by` do not match
    ///
    /// This is an advanced feature, `ContiguousComponentTicksMut`s are designed to be _created_ by
    /// engine-internal code and _consumed_ by end-user code.
    ///
    /// - `added` - [`Tick`]s that store the tick when the wrapped value was created.
    /// - `changed` - [`Tick`]s that store the last time the wrapped value was changed.
    /// - `last_run` - A [`Tick`], occurring before `this_run`, which is used
    ///   as a reference to determine whether the wrapped value is newly added or changed.
    /// - `this_run` - A [`Tick`] corresponding to the current point in time -- "now".
    /// - `caller` - [`Location`]s that store the location when the wrapper value was changed.
    pub fn new(
        added: &'w mut [Tick],
        changed: &'w mut [Tick],
        last_run: Tick,
        this_run: Tick,
        caller: MaybeLocation<&'w mut [&'static Location<'static>]>,
    ) -> Option<Self> {
        let eq = added.len() == changed.len()
            && caller
                .as_ref()
                .map(|v| v.len() == added.len())
                .into_option()
                .unwrap_or(true);
        eq.then_some(Self {
            added,
            changed,
            changed_by: caller,
            last_run,
            this_run
        })
    }

    /// Returns added ticks' slice.
    pub fn added(&self) -> &[Tick] {
        self.added
    }

    /// Returns changed ticks' slice.
    pub fn changed(&self) -> &[Tick] {
        self.changed
    }

    /// Returns changed by locations' slice.
    pub fn changed_by(&self) -> MaybeLocation<&[&'static Location<'static>]> {
        self.changed_by.as_deref()
    }

    /// Returns mutable added ticks' slice.
    pub fn added_mut(&mut self) -> &mut [Tick] {
        self.added
    }

    /// Returns mutable changed ticks' slice.
    pub fn changed_mut(&mut self) -> &mut [Tick] {
        self.changed
    }

    /// Returns mutable changed by locations' slice.
    pub fn changed_by_mut(&mut self) -> MaybeLocation<&mut [&'static Location<'static>]> {
        self.changed_by.as_deref_mut()
    }

    /// Returns the tick the system last ran.
    pub fn last_run(&self) -> Tick {
        self.last_run
    }

    /// Returns the tick of the current system's run.
    pub fn this_run(&self) -> Tick {
        self.this_run
    }

    /// Returns an iterator where the i-th item corresponds to whether the i-th component was
    /// marked as changed. If the value equals [`prim@true`], then the component was changed.
    ///
    /// # Example
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct A(pub i32);
    ///
    /// fn some_system(mut query: Query<&mut A>) {
    ///     for a in query.contiguous_iter_mut().unwrap() {
    ///         let (a_values, a_ticks) = ContiguousMut::split(a);
    ///         for (value, is_changed) in a_values.iter_mut().zip(a_ticks.is_changed_iter()) {
    ///             if is_changed {
    ///                 value.0 *= 10;
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub fn is_changed_iter(&self) -> impl Iterator<Item = bool> {
        self.changed
            .iter()
            .map(|v| v.is_newer_than(self.last_run, self.this_run))
    }

    /// Returns an iterator where the i-th item corresponds to whether the i-th component was
    /// marked as added. If the value equals [`prim@true`], then the component was added.
    ///
    /// # Example
    /// ```
    /// # use bevy_ecs::prelude::*;
    /// #
    /// # #[derive(Component)]
    /// # struct A(pub i32);
    ///
    /// fn some_system(mut query: Query<&mut A>) {
    ///     for a in query.contiguous_iter_mut().unwrap() {
    ///         let (a_values, a_ticks) = ContiguousMut::split(a);
    ///         for (value, is_added) in a_values.iter_mut().zip(a_ticks.is_added_iter()) {
    ///             if is_added {
    ///                 value.0 = 10;
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    pub fn is_added_iter(&self) -> impl Iterator<Item = bool> {
        self.added
            .iter()
            .map(|v| v.is_newer_than(self.last_run, self.this_run))
    }

    /// Marks every tick as changed.
    pub fn mark_all_as_changed(&mut self) {
        let this_run = self.this_run;

        self.changed_by.as_mut().map(|v| {
            for v in v.iter_mut() {
                *v = Location::caller()
            }
        });

        for t in self.changed.iter_mut() {
            *t = this_run
        }
    }

    /// Returns a `ContiguousComponentTicksMut` with a smaller lifetime.
    pub fn reborrow(&mut self) -> ContiguousComponentTicksMut<'_> {
        ContiguousComponentTicksMut {
            added: self.added,
            changed: self.changed,
            changed_by: self.changed_by.as_deref_mut(),
            last_run: self.last_run,
            this_run: self.this_run
        }
    }
}

// TODO!
