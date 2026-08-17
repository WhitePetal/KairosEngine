use crate::ecs::{
    component::Component,
    entity::{ComponentCloneCtx, SourceComponent},
};

/// Function type that can be used to clone a component of an entity.
pub type ComponentCloneFn = fn(&SourceComponent, &mut ComponentCloneCtx);

/// The clone behavior to use when cloning or moving a [`Component`].
#[derive(Clone, Debug, Default)]
pub enum ComponentCloneBehavior {
    /// Uses the default behavior (which is passed to [`ComponentCloneBehavior::resolve`])
    #[default]
    Default,
    /// Do not clone/move this component.
    Ignore,
    /// Uses a custom [`ComponentCloneFn`].
    Custom(ComponentCloneFn),
}

impl ComponentCloneBehavior {
    /// Set clone handler based on `Clone` trait.
    ///
    /// If set as a handler for a component that is not the same as the one used to create this handler, it will panic.
    pub fn clone<C: Component + Clone>() -> Self {
        Self::Custom(component_clone_via_clone::<C>)
    }

    pub fn global_default_fn() -> ComponentCloneFn {
        // #[cfg(feature = "bevy_reflect")]
        // return component_clone_via_reflect;
        // #[cfg(not(feature = "bevy_reflect"))]
        return component_clone_ignore;
    }
}

/// Component [clone handler function](ComponentCloneFn) implemented using the [`Clone`] trait.
/// Can be [set](Component::clone_behavior) as clone handler for the specific component it is implemented for.
/// It will panic if set as handler for any other component.
///
pub fn component_clone_via_clone<C: Clone + Component>(
    source: &SourceComponent,
    ctx: &mut ComponentCloneCtx,
) {
    if let Some(component) = source.read::<C>() {
        ctx.write_target_component(component.clone());
    }
}

pub fn component_clone_ignore(_source: &SourceComponent, _ctx: &mut ComponentCloneCtx) {}
