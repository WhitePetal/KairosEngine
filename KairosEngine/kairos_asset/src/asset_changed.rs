//! The [`AssetChanged`] query filter.
//!
//! Like [`Changed`](kairos_ecs::prelude::Changed), but for [`Asset`]s: it matches
//! an entity whose component implements [`AsAssetId`] whenever the asset behind
//! that component changed — even when the component itself never did.
//!
//! This is a **filter-only** [`WorldQuery`]: it deliberately does not implement
//! [`QueryData`], matching `bevy_asset` 0.19.1. As with `bevy_asset`, the
//! underlying [`AssetChanges`] resource is only created when an `AssetChanged`
//! query is initialized, so a program that never uses the filter pays nothing.

use core::marker::PhantomData;

use kairos_collections::FixedHashMap as HashMap;
use kairos_ecs::{
    archetype::Archetype,
    change_detection::Tick,
    component::{Component, ComponentId, Components},
    debug::DebugName,
    entity::Entity,
    query::{FilteredAccess, FilteredAccessSet, QueryData, QueryFilter, ReadFetch, WorldQuery},
    resource::{IS_RESOURCE, Resource},
    storage::{Table, TableRow},
    world::{World, unsafe_world_cell::UnsafeWorldCell},
};
use tracing::error;

use crate::asset::Asset;
use crate::id::AssetId;

/// A trait for components that can be used as asset identifiers, e.g. handle
/// wrappers.
///
/// Implementing this lets [`AssetChanged`] look through the component to the
/// [`AssetId`] it names, so the filter reacts to the asset changing rather than
/// to the component itself changing.
pub trait AsAssetId: Component {
    /// The underlying asset type.
    type Asset: Asset;

    /// Retrieves the asset id from this component.
    fn as_asset_id(&self) -> AssetId<Self::Asset>;
}

/// A resource that stores the last tick an asset was changed. This is used by
/// the [`AssetChanged`] filter to determine if an asset has changed since the
/// last time a query ran.
///
/// This resource is automatically managed by the
/// [`AssetEventSystems`](crate::AssetEventSystems) system set and is not part of
/// the public API in order to maintain safety guarantees. It only exists once an
/// [`AssetChanged`] query has been initialized; any other use should be carefully
/// audited to ensure it does not introduce safety issues.
#[derive(Resource)]
pub(crate) struct AssetChanges<A: Asset> {
    change_ticks: HashMap<AssetId<A>, Tick>,
    last_change_tick: Tick,
}

impl<A: Asset> AssetChanges<A> {
    pub(crate) fn insert(&mut self, asset_id: AssetId<A>, tick: Tick) {
        self.last_change_tick = tick;
        self.change_ticks.insert(asset_id, tick);
    }

    pub(crate) fn remove(&mut self, asset_id: &AssetId<A>) {
        self.change_ticks.remove(asset_id);
    }
}

impl<A: Asset> Default for AssetChanges<A> {
    fn default() -> Self {
        Self {
            change_ticks: Default::default(),
            last_change_tick: Tick::new(0),
        }
    }
}

struct AssetChangeCheck<'w, A: AsAssetId> {
    // This should never be `None` in practice, but we need to handle the case
    // where the `AssetChanges` resource was removed.
    change_ticks: Option<&'w HashMap<AssetId<A::Asset>, Tick>>,
    last_run: Tick,
    this_run: Tick,
}

impl<A: AsAssetId> Clone for AssetChangeCheck<'_, A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: AsAssetId> Copy for AssetChangeCheck<'_, A> {}

impl<'w, A: AsAssetId> AssetChangeCheck<'w, A> {
    fn new(changes: &'w AssetChanges<A::Asset>, last_run: Tick, this_run: Tick) -> Self {
        Self {
            change_ticks: Some(&changes.change_ticks),
            last_run,
            this_run,
        }
    }
    // TODO(perf): some sort of caching? Each check has two levels of indirection,
    // which is not optimal.
    fn has_changed(&self, handle: &A) -> bool {
        let is_newer = |tick: &Tick| tick.is_newer_than(self.last_run, self.this_run);
        let id = handle.as_asset_id();

        self.change_ticks
            .is_some_and(|change_ticks| change_ticks.get(&id).is_some_and(is_newer))
    }
}

/// Filter that selects entities with an `A` for an asset that changed after the
/// system last ran, where `A` is a component that implements [`AsAssetId`].
///
/// Unlike `Changed<A>`, this is true whenever the asset for the `A` in
/// `ResMut<Assets<A>>` changed. For example, when a mesh changed through the
/// [`Assets<Mesh>::get_mut`](crate::Assets::get_mut) method, `AssetChanged<Mesh>`
/// will iterate over all entities with the `Handle<Mesh>` for that mesh.
/// Meanwhile, `Changed<Handle<Mesh>>` will iterate over no entities.
///
/// Swapping the actual `A` component is a common pattern. So you should check for
/// _both_ `AssetChanged<A>` and `Changed<A>` with
/// `Or<(Changed<A>, AssetChanged<A>)>`.
///
/// # Quirks
///
/// - Asset changes are registered in the [`AssetEventSystems`](crate::AssetEventSystems) system set.
/// - Removed assets are not detected.
///
/// The list of changed assets only gets updated in the
/// [`AssetEventSystems`](crate::AssetEventSystems) system set. Therefore,
/// `AssetChanged` will only pick up asset changes in schedules following that
/// set or the next frame.
///
/// # Performance
///
/// When at least one `A` is updated, this will read a hashmap once per entity
/// with an `A` component. The runtime of the query is proportional to how many
/// entities with an `A` it matches.
///
/// If no `A` asset updated since the last time the system ran, then no lookups
/// occur.
pub struct AssetChanged<A: AsAssetId>(PhantomData<A>);

/// [`WorldQuery`] fetch for [`AssetChanged`].
#[doc(hidden)]
pub struct AssetChangedFetch<'w, A: AsAssetId> {
    inner: Option<ReadFetch<'w, A>>,
    check: AssetChangeCheck<'w, A>,
}

impl<'w, A: AsAssetId> Clone for AssetChangedFetch<'w, A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner,
            check: self.check,
        }
    }
}

/// [`WorldQuery`] state for [`AssetChanged`].
#[doc(hidden)]
pub struct AssetChangedState<A: AsAssetId> {
    asset_id: ComponentId,
    resource_id: ComponentId,
    _asset: PhantomData<fn(A)>,
}

#[expect(unsafe_code, reason = "WorldQuery is an unsafe trait.")]
// SAFETY: `ROQueryFetch<Self>` is the same as `QueryFetch<Self>`
unsafe impl<A: AsAssetId> WorldQuery for AssetChanged<A> {
    type Fetch<'w> = AssetChangedFetch<'w, A>;

    type State = AssetChangedState<A>;

    fn shrink_fetch<'wlong: 'wshort, 'wshort>(fetch: Self::Fetch<'wlong>) -> Self::Fetch<'wshort> {
        fetch
    }

    unsafe fn init_fetch<'w, 's>(
        world: UnsafeWorldCell<'w>,
        state: &'s Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w> {
        // SAFETY:
        // - `state.resource_id` was obtained from `world.init_resource::<AssetChanges<A::Asset>>()`,
        //   so the untyped pointer returned by `get_resource_by_id` can safely be dereferenced into that type.
        // - `init_nested_access` declares a read on `state.resource_id`, so it is safe to
        //   read that resource here (see trait-level safety comments on `WorldQuery`)
        let Some(changes) = (unsafe {
            world
                .get_resource_by_id(state.resource_id)
                .map(|ptr| ptr.deref::<AssetChanges<A::Asset>>())
        }) else {
            error!(
                "AssetChanges<{ty}> resource was removed, please do not remove \
                AssetChanges<{ty}> when using the AssetChanged<{ty}> world query",
                ty = DebugName::type_name::<A>(),
            );

            return AssetChangedFetch {
                inner: None,
                check: AssetChangeCheck {
                    change_ticks: None,
                    last_run,
                    this_run,
                },
            };
        };
        let has_updates = changes.last_change_tick.is_newer_than(last_run, this_run);

        AssetChangedFetch {
            inner: has_updates.then(||
                // SAFETY: We delegate to the inner `init_fetch` for `A`
                unsafe { <&A>::init_fetch(world, &state.asset_id, last_run, this_run) }),
            check: AssetChangeCheck::new(changes, last_run, this_run),
        }
    }

    const IS_DENSE: bool = <&A>::IS_DENSE;

    unsafe fn set_archetype<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        state: &'s Self::State,
        archetype: &'w Archetype,
        table: &'w Table,
    ) {
        if let Some(inner) = &mut fetch.inner {
            // SAFETY: We delegate to the inner `set_archetype` for `A`
            unsafe {
                <&A>::set_archetype(inner, &state.asset_id, archetype, table);
            }
        }
    }

    unsafe fn set_table<'w, 's>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        table: &'w Table,
    ) {
        if let Some(inner) = &mut fetch.inner {
            // SAFETY: We delegate to the inner `set_table` for `A`
            unsafe {
                <&A>::set_table(inner, &state.asset_id, table);
            }
        }
    }

    #[inline]
    fn update_component_access(state: &Self::State, access: &mut FilteredAccess) {
        <&A>::update_component_access(&state.asset_id, access);
    }

    // AssetChanged accesses both the asset and the AssetChanges<A> resource.
    // In order to access two different entities we implement init_nested_access.
    fn init_nested_access(
        state: &Self::State,
        system_name: Option<&str>,
        component_access_set: &mut FilteredAccessSet,
        _world: UnsafeWorldCell,
    ) {
        let mut filter = FilteredAccess::default();
        filter.add_read(state.resource_id);
        filter.and_with(IS_RESOURCE);

        let conflicts = component_access_set.get_conflicts_single(&filter);
        if conflicts.is_empty() {
            component_access_set.add(filter);
            return;
        }
        panic!(
            "error[B0002]: AssetChanged<{}> in system {:?} conflicts with a previous system parameter. Consider removing the duplicate access. See: https://bevy.org/learn/errors/b0002",
            DebugName::type_name::<A>(),
            system_name
        );
    }

    fn init_state(world: &mut World) -> AssetChangedState<A> {
        let resource_id = world.init_resource::<AssetChanges<A::Asset>>();
        let asset_id = world.register_component::<A>();
        AssetChangedState {
            asset_id,
            resource_id,
            _asset: PhantomData,
        }
    }

    fn get_state(components: &Components) -> Option<Self::State> {
        let resource_id = components.component_id::<AssetChanges<A::Asset>>()?;
        let asset_id = components.component_id::<A>()?;
        Some(AssetChangedState {
            asset_id,
            resource_id,
            _asset: PhantomData,
        })
    }

    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        set_contains_id(state.asset_id)
    }
}

#[expect(unsafe_code, reason = "QueryFilter is an unsafe trait.")]
// SAFETY: read-only access
unsafe impl<A: AsAssetId> QueryFilter for AssetChanged<A> {
    const IS_ARCHETYPAL: bool = false;

    #[inline]
    unsafe fn filter_fetch(
        state: &Self::State,
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        fetch.inner.as_mut().is_some_and(|inner| {
            // SAFETY: We delegate to the inner `fetch` for `A`
            unsafe {
                let handle = <&A>::fetch(&state.asset_id, inner, entity, table_row);
                handle.is_some_and(|handle| fetch.check.has_changed(handle))
            }
        })
    }
}
