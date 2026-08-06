use std::{cell::UnsafeCell, ops::{Deref, DerefMut}, panic::Location};

use crate::{
    debug::MaybeLocation,
    ecs::{
        change_detection::{
            ComponentTickCells, DetectChanges, Tick,
            traits::*,
        },
        component::Mutable,
        resource::Resource,
    },
    ptr::{ThinSlicePtr, UnsafeCellDeref},
};

/// Used by immutable query parameters (such as [`Ref`] and [`Res`])
/// to store immutable access to the [`Tick`]s of a single component or resource.
#[derive(Clone, Copy)]
pub(crate) struct ComponentTicksRef<'w> {
    pub(crate) added: &'w Tick,
    pub(crate) changed: &'w Tick,
    pub(crate) changed_by: MaybeLocation<&'w &'static Location<'static>>,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

impl<'w> ComponentTicksRef<'w> {
    /// # Safety
    /// This should never alias the underlying ticks with a mutable one such as `ComponentTicksMut`.
    #[inline]
    pub(crate) unsafe fn from_tick_cells(
        cells: ComponentTickCells<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        Self {
            added: unsafe { cells.added.deref() },
            changed: unsafe { cells.changed.deref() },
            changed_by: unsafe { cells.changed_by.map(|changed_by| changed_by.deref()) },
            last_run,
            this_run,
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
        last_run: Tick,
    ) -> Self {
        Self {
            // SAFETY:
            // - The caller ensures that `len` is the length of the slice.
            // - The caller ensures we have permission to read the data.
            added: unsafe { added.cast().as_slice_unchecked(len) },
            // SAFETY: see above.
            changed: unsafe { changed.cast().as_slice_unchecked(len) },
            // SAFETY: see above.
            changed_by: change_by.map(|v| unsafe { v.cast().as_slice_unchecked(len) }),
            last_run,
            this_run,
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
        caller: MaybeLocation<&'w [&'static Location<'static>]>,
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
            this_run,
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
        this_run: Tick,
    ) -> Self {
        Self {
            added: unsafe { cells.added.deref_mut() },
            changed: unsafe { cells.changed.deref_mut() },
            changed_by: unsafe { cells.changed_by.map(|changed_by| changed_by.deref_mut()) },
            last_run,
            this_run,
        }
    }
}

impl<'w> From<ComponentTicksMut<'w>> for ComponentTicksRef<'w> {
    fn from(ticks: ComponentTicksMut<'w>) -> Self {
        ComponentTicksRef {
            added: ticks.added,
            changed: ticks.changed,
            changed_by: ticks.changed_by.map(|changed_by| &*changed_by),
            last_run: ticks.last_run,
            this_run: ticks.this_run,
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
        last_run: Tick,
    ) -> Self {
        Self {
            // SAFETY:
            // - The caller ensures that `len` is the length of the slice.
            // - The caller ensures we have permission to mutate the data.
            added: unsafe { added.as_mut_slice_unchecked(len) },
            // SAFETY: see above.
            changed: unsafe { changed.as_mut_slice_unchecked(len) },
            // SAFETY: see above.
            changed_by: changed_by.map(|v| unsafe { v.as_mut_slice_unchecked(len) }),
            last_run,
            this_run,
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
            this_run,
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
            this_run: self.this_run,
        }
    }
}

impl<'w> From<ContiguousComponentTicksMut<'w>> for ContiguousComponentTicksRef<'w> {
    fn from(value: ContiguousComponentTicksMut<'w>) -> Self {
        Self {
            added: value.added,
            changed: value.changed,
            changed_by: value.changed_by.map(|v| &*v),
            last_run: value.last_run,
            this_run: value.this_run,
        }
    }
}

/// Shared borrow of a [`Resource`].
///
/// See the [`Resource`] documentation for usage.
///
/// If you need a unique mutable borrow, use [`ResMut`] instead.
///
/// This [`SystemParam`](crate::system::SystemParam) fails validation if resource doesn't exist.
/// This will cause a panic, but can be configured to do nothing or warn once.
///
/// Use [`Option<Res<T>>`] instead if the resource might not always exist.
pub struct Res<'w, T: ?Sized + Resource> {
    pub(crate) value: &'w T,
    pub(crate) ticks: ComponentTicksRef<'w>,
}

impl<'w, T: Resource> Res<'w, T> {
    /// Copies a reference to a resource.
    ///
    /// Note that unless you actually need an instance of `Res<T>`, you should
    /// prefer to just convert it to `&T` which can be freely copied.
    #[expect(
        clippy::should_implement_trait,
        reason = "As this struct derefs to the inner resource, a `Clone` trait implementation would interfere with the common case of cloning the inner content."
    )]
    pub fn clone(this: &Self) -> Self {
        Self {
            value: this.value,
            ticks: this.ticks,
        }
    }

    /// Due to lifetime limitations of the `Deref` trait, this method can be used to obtain a
    /// reference of the [`Resource`] with a lifetime bound to `'w` instead of the lifetime of the
    /// struct itself.
    pub fn into_inner(self) -> &'w T {
        self.value
    }
}

impl<'w, T: Resource<Mutability = Mutable>> From<ResMut<'w, T>> for Res<'w, T> {
    fn from(res: ResMut<'w, T>) -> Self {
        Self {
            value: res.value,
            ticks: res.ticks.into(),
        }
    }
}

impl<'w, T: Resource> From<Res<'w, T>> for Ref<'w, T> {
    /// Convert a `Res` into a `Ref`. This allows keeping the change-detection feature of `Ref`
    /// while losing the specificity of `Res` for resources.
    fn from(res: Res<'w, T>) -> Self {
        Self {
            value: res.value,
            ticks: res.ticks,
        }
    }
}

impl<'w, 'a, T: Resource> IntoIterator for &'a Res<'w, T>
where
    &'a T: IntoIterator,
{
    type Item = <&'a T as IntoIterator>::Item;

    type IntoIter = <&'a T as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.value.into_iter()
    }
}

change_detection_impl!(Res<'w, T>, T, Resource);
impl_debug!(Res<'w, T>, Resource);

/// Unique mutable borrow of a [`Resource`].
///
/// See the [`Resource`] documentation for usage.
///
/// If you need a shared borrow, use [`Res`] instead.
///
/// This [`SystemParam`](crate::system::SystemParam) fails validation if resource doesn't exist.
/// This will cause a panic, but can be configured to do nothing or warn once.
///
/// Use [`Option<ResMut<T>>`] instead if the resource might not always exist.
pub struct ResMut<'w, T: ?Sized + Resource<Mutability = Mutable>> {
    pub(crate) value: &'w mut T,
    pub(crate) ticks: ComponentTicksMut<'w>,
}

change_detection_impl!(ResMut<'w, T>, T, Resource<Mutability = Mutable>);
change_detection_mut_impl!(ResMut<'w, T>, T, Resource<Mutability = Mutable>);
impl_methods!(ResMut<'w, T>, T, Resource<Mutability = Mutable>);
impl_debug!(ResMut<'w, T>, Resource<Mutability = Mutable>);

impl<'w, 'a, T: Resource<Mutability = Mutable>> IntoIterator for &'a ResMut<'w, T>
where
    &'a T: IntoIterator,
{
    type Item = <&'a T as IntoIterator>::Item;

    type IntoIter = <&'a T as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {

        self.value.into_iter()
    }
}

impl<'w, 'a, T: Resource<Mutability = Mutable>> IntoIterator for &'a mut ResMut<'w, T> where &'a mut T: IntoIterator {
    type Item = <&'a mut T as IntoIterator>::Item;

    type IntoIter = <&'a mut T as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.set_changed();
        self.value.into_iter()
    }
}

impl<'w, T: Resource<Mutability = Mutable>> From<ResMut<'w, T>> for Mut<'w, T> {
    /// Convert this `ResMut` into a `Mut`. This allows keeping the change-detection feature of `Mut`
    /// while losing the specificity of `ResMut` for resources.
    fn from(other: ResMut<'w, T>) -> Self {
        Self {
            value: other.value,
            ticks: other.ticks
        }
    }
}

/// Shared borrow of a non-[`Send`] resource.
///
/// Only [`Send`] resources may be accessed with the [`Res`] [`SystemParam`](crate::system::SystemParam). In case that the
/// resource does not implement `Send`, this `SystemParam` wrapper can be used. This will instruct
/// the scheduler to instead run the system on the main thread so that it doesn't send the resource
/// over to another thread.
///
/// This [`SystemParam`](crate::system::SystemParam) fails validation if the non-send resource doesn't exist.
/// This will cause a panic, but can be configured to do nothing or warn once.
///
/// Use [`Option<NonSend<T>>`] instead if the resource might not always exist.
pub struct NonSend<'w, T: ?Sized + 'static> {
    pub(crate) value: &'w T,
    pub(crate) ticks: ComponentTicksRef<'w>,
}

change_detection_impl!(NonSend<'w, T>, T,);
impl_debug!(NonSend<'w, T>,);

impl<'w, T> From<NonSendMut<'w, T>> for NonSend<'w, T> {
    fn from(other: NonSendMut<'w, T>) -> Self {
        Self {
            value: other.value,
            ticks: other.ticks.into(),
        }
    }
}

/// Unique borrow of a non-[`Send`] resource.
///
/// Only [`Send`] resources may be accessed with the [`ResMut`] [`SystemParam`](crate::system::SystemParam). In case that the
/// resource does not implement `Send`, this `SystemParam` wrapper can be used. This will instruct
/// the scheduler to instead run the system on the main thread so that it doesn't send the resource
/// over to another thread.
///
/// This [`SystemParam`](crate::system::SystemParam) fails validation if non-send resource doesn't exist.
/// This will cause a panic, but can be configured to do nothing or warn once.
///
/// Use [`Option<NonSendMut<T>>`] instead if the resource might not always exist.
pub struct NonSendMut<'w, T: ?Sized + 'static> {
    pub(crate) value: &'w mut T,
    pub(crate) ticks: ComponentTicksMut<'w>,
}

change_detection_impl!(NonSendMut<'w, T>, T,);
change_detection_mut_impl!(NonSendMut<'w, T>, T,);
impl_methods!(NonSendMut<'w, T>, T,);
impl_debug!(NonSendMut<'w, T>,);

impl<'w, T: 'static> From<NonSendMut<'w, T>> for Mut<'w, T> {
    /// Convert this `NonSendMut` into a `Mut`. This allows keeping the change-detection feature of `Mut`
    /// while losing the specificity of `NonSendMut`.
    fn from(other: NonSendMut<'w, T>) -> Self {
        Mut {
            value: other.value,
            ticks: other.ticks
        }
    }
}

/// Shared borrow of an entity's component with access to change detection.
/// Similar to [`Mut`] but is immutable and so doesn't require unique access.
///
/// # Examples
///
/// These two systems produce the same output.
///
/// ```
/// # use bevy_ecs::change_detection::DetectChanges;
/// # use bevy_ecs::query::{Changed, With};
/// # use bevy_ecs::system::Query;
/// # use bevy_ecs::world::Ref;
/// # use bevy_ecs_macros::Component;
/// # #[derive(Component)]
/// # struct MyComponent;
///
/// fn how_many_changed_1(query: Query<(), Changed<MyComponent>>) {
///     println!("{} changed", query.iter().count());
/// }
///
/// fn how_many_changed_2(query: Query<Ref<MyComponent>>) {
///     println!("{} changed", query.iter().filter(|c| c.is_changed()).count());
/// }
/// ```
pub struct Ref<'w, T: ?Sized> {
    pub(crate) value: &'w T,
    pub(crate) ticks: ComponentTicksRef<'w>,
}

/// Unique mutable borrow of an entity's component or of a resource.
///
/// This can be used in queries to access change detection from immutable query methods, as opposed
/// to `&mut T` which only provides access to change detection from mutable query methods.
///
/// ```rust
/// # use bevy_ecs::prelude::*;
/// # use bevy_ecs::query::QueryData;
/// #
/// #[derive(Component, Clone, Debug)]
/// struct Name(String);
///
/// #[derive(Component, Clone, Copy, Debug)]
/// struct Health(f32);
///
/// fn my_system(mut query: Query<(Mut<Name>, &mut Health)>) {
///     // Mutable access provides change detection information for both parameters:
///     // - `name` has type `Mut<Name>`
///     // - `health` has type `Mut<Health>`
///     for (name, health) in query.iter_mut() {
///         println!("Name: {:?} (last changed {:?})", name, name.last_changed());
///         println!("Health: {:?} (last changed: {:?})", health, health.last_changed());
/// #        println!("{}{}", name.0, health.0); // Silence dead_code warning
///     }
///
///     // Immutable access only provides change detection for `Name`:
///     // - `name` has type `Ref<Name>`
///     // - `health` has type `&Health`
///     for (name, health) in query.iter() {
///         println!("Name: {:?} (last changed {:?})", name, name.last_changed());
///         println!("Health: {:?}", health);
///     }
/// }
///
/// # bevy_ecs::system::assert_is_system(my_system);
/// ```
pub struct Mut<'w, T: ?Sized> {
    pub(crate) value: &'w mut T,
    pub(crate) ticks: ComponentTicksMut<'w>,
}

// TODO!
