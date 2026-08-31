use crate::ecs::{bundle::Bundle, relationship::Relationship, world::EntityWorldMut};



impl<'w> EntityWorldMut<'w> {
    /// Spawns a entity related to this entity (with the `R` relationship) by taking a bundle
    pub fn with_related<R: Relationship>(&mut self, bundle: impl Bundle) -> &mut Self {
        let parent = self.id();
        self.world_scope(|world| {
            world.spawn((bundle, R::from(parent)));
        });
        self
    }
}
