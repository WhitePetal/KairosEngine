use std::sync::Arc;

use indexmap::IndexMap;

use crate::{
    debug::MaybeLocation,
    ecs::{
        change_detection::Tick,
        component::{Component, ComponentId, Components, ComponentsRegistrator},
        entity::Entity,
        storage::{SparseSets, Table, TableRow},
    },
    hash::FixedHasher,
    ptr::OwningPtr,
};

/// Metadata associated with a required component. See [`Component`] for details.
#[derive(Clone)]
pub struct RequiredComponent {
    /// The constructor used for the required component.
    pub constructor: RequiredComponentConstructor,
}

/// A Required Component constructor. See [`Component`] for details.
#[derive(Clone)]
pub struct RequiredComponentConstructor(
    // Note: this function makes `unsafe` assumptions, so it cannot be public.
    Arc<dyn Fn(&mut Table, &mut SparseSets, Tick, TableRow, Entity, MaybeLocation)>,
);

impl RequiredComponentConstructor {
    /// Creates a new instance of `RequiredComponentConstructor` for the given type
    ///
    /// # Safety
    ///
    /// - `component_id` must be a valid component for type `C`.
    pub unsafe fn new<C: Component>(
        component_id: ComponentId,
        constructor: impl Fn() -> C + 'static,
    ) -> Self {
        RequiredComponentConstructor({
            type Constructor = dyn for<'a, 'b> Fn(
                &'a mut Table,
                &'b mut SparseSets,
                Tick,
                TableRow,
                Entity,
                MaybeLocation,
            );

            type Intermediate<T> = Arc<T>;

            let boxed: Intermediate<Constructor> = Intermediate::new(
                move |table, sparse_sets, change_tick, table_row, entity, caller| {
                    OwningPtr::make(constructor(), |ptr| {
                        // SAFETY: This will only be called in the context of `BundleInfo::write_components`, which will
                        // pass in a valid table_row and entity requiring a C constructor
                        // C::STORAGE_TYPE is the storage type associated with `component_id` / `C`
                        // `ptr` points to valid `C` data, which matches the type associated with `component_id`
                        unsafe { todo!() }
                    });
                },
            );

            Arc::from(boxed)
        })
    }
}

/// The collection of metadata for components that are required for a given component.
///
/// For more information, see the "Required Components" section of [`Component`].
#[derive(Default, Clone)]
pub struct RequiredComponents {
    /// The components that are directly required (i.e. excluding inherited ones), in the order of their precedence.
    ///
    /// # Safety
    /// The [`RequiredComponent`] instance associated to each ID must be valid for its component.
    pub(crate) direct: IndexMap<ComponentId, RequiredComponent, FixedHasher>,
    /// All the components that are required (i.e. including inherited ones), in depth-first order. Most importantly,
    /// components in this list always appear after all the components that they require.
    ///
    /// Note that the direct components are not necessarily at the end of this list, for example if A and C are directly
    /// requires, and A requires B requires C, then `all` will hold [C, B, A].
    ///
    /// # Safety
    /// The [`RequiredComponent`] instance associated to each ID must be valid for its component.
    pub(crate) all: IndexMap<ComponentId, RequiredComponent, FixedHasher>,
}

/// This is a safe handle around `ComponentsRegistrator` and `RequiredComponents` to register required components.
pub struct RequiredComponentsRegistrator<'a, 'w> {
    components: &'a mut ComponentsRegistrator<'w>,
    required_components: &'a mut RequiredComponents,
}

impl<'a, 'w> RequiredComponentsRegistrator<'a, 'w> {
    /// # Safety
    ///
    /// All components in `required_components` must have been registered in `components`
    pub(crate) unsafe fn new(
        components: &'a mut ComponentsRegistrator<'w>,
        required_components: &'a mut RequiredComponents,
    ) -> Self {
        Self {
            components,
            required_components,
        }
    }
}

pub(super) fn enforce_no_required_components_recursion(
    components: &Components,
    recursion_check_stack: &[ComponentId],
    required: ComponentId,
) {
    if let Some(direct_recursion) = recursion_check_stack
        .iter()
        .position(|&id| id == required)
        .map(|index| index == recursion_check_stack.len() - 1)
    {
        panic!(
            "Recursive required components detected: {}\nhelp: {}",
            recursion_check_stack
                .iter()
                .map(|id| format!("{}", components.get_name(*id).unwrap().shortname()))
                .collect::<Vec<_>>()
                .join(" -> "),
            if direct_recursion {
                format!(
                    "Remove require({}).",
                    components.get_name(required).unwrap().shortname()
                )
            } else {
                "If this is intentional, consider merging the components.".into()
            }
        );
    }
}
