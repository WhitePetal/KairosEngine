

#[derive(Clone, Debug, Default)]
pub(crate) enum MaybeRelationshipAccessor {
    /// Not a relationship
    #[default]
    NoAccessor,
    /// Uninitialized relationship, which will be initialized when the second component of the relationship is registered.
    /// Boxed to reduce size overhead.
    Initializer(Box<RelationshipAccessorInitializer>),
    /// Relationship
    Accessor(RelationshipAccessor),
}

//TODO!
