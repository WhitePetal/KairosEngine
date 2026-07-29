

use std::sync::RwLock;

use crate::collections::TypeIdMap;



/// Stores metadata associated with each kind of [`Component`] in a given [`World`](crate::world::World).
#[derive(Debug, Default)]
pub struct Components {
    pub(super) components: Vec<Option<ComponentInfo>>,
    pub(super) indices: TypeIdMap<ComponentId>,
    // This is kept internal and local to verify that no deadlocks can occur.
    pub(super) queued: RwLock<QueuedCommponents>,
}
