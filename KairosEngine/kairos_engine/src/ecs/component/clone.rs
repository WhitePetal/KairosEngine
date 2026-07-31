/// Function type that can be used to clone a component of an entity.
pub type ComponentCloneFn = fn();

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

// TODO!
