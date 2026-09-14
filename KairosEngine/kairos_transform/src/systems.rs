use std::sync::atomic::AtomicU64;

use kairos_ecs::{change_detection::{DetectChanges, Ref}, entity::Entity, hierarchy::{ChildOf, Children}, lifecycle::RemovedComponents, query::{Added, Changed, Or, QueryFilter, Without}, resource::Resource, system::{Local, ParamSet, Query, Res}};
use kairos_tasks::parallel_queue::Parallel;

use crate::{GlobalTransform, LocalTransform, TransformHelper, TransformTreeChanged};


/// Generic system that propagates transforms,
/// using [`TransformHelper`] for any entity matching the filter `F`.
/// Useful for moving and rendering in the same frame.
pub fn propagate_transforms_for<F: QueryFilter + 'static>(
    tf_helper: TransformHelper,
    mut query: Query<(Entity, &mut GlobalTransform), F>,
) {
    for (entity, mut gtf) in query.iter_mut() {
        let result = tf_helper
            .compute_global_transform(entity)
            .inspect_err(|_err| {
                #[cfg(feature = "trace")]
                tracing::warn!(
                    "Failed to compute GlobalTransform for entity {:?}: {:?}",
                    entity,
                    _err
                );
            });

        if let Ok(computed) = result {
            *gtf = computed;
        }
    }
}

/// Update [`GlobalTransform`] component of entities that aren't in the hierarchy
///
/// Third party plugins should ensure that this is used in concert with
/// [`propagate_parent_transforms`] and [`mark_dirty_trees`].
pub fn sync_simple_transforms(
    mut query: ParamSet<(
        Query<
            (&LocalTransform, &mut GlobalTransform),
            (
                Or<(Changed<LocalTransform>, Added<GlobalTransform>)>,
                Without<ChildOf>,
                Without<Children>
            )
        >,
        Query<
            (Ref<LocalTransform>, &mut GlobalTransform),
            (Without<ChildOf>, Without<Children>)
        >,
    )>,
    mut orphaned: RemovedComponents<ChildOf>
) {
    query
        .p0()
        .par_iter_mut()
        .for_each(|(transform, mut global_transform)| {
            *global_transform = GlobalTransform::from(*transform)
        });

    // Update orphaned entities.
    let mut query = query.p1();
    let mut iter = query.iter_many_mut(orphaned.read());
    while let Some((transform, mut global_transform)) = iter.fetch_next() {
        if !transform.is_changed() && !global_transform.is_added() {
            *global_transform = GlobalTransform::from(*transform)
        }
    }
}

/// Configure the behavior of static scene optimizations for [`Transform`] propagation.
///
/// For scenes with many static entities, it is much faster to track trees of unchanged
/// [`LocalTransform`]s and skip these during the expensive transform propagation step. If your scene is
/// very dynamic, the cost of tracking these trees can exceed the performance benefits. By default,
/// static scene optimization is enabled.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub enum StaticTransformOptimizations {
    /// Enable static scene optimizations.
    #[default]
    Enabled,
    /// Disable static scene optimizations.
    Disabled,
}

impl StaticTransformOptimizations {
    /// Returns `true` if static scene optimizations are enabled.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        *self == StaticTransformOptimizations::Enabled
    }
}

pub fn mark_dirty_trees(
    changed: Query<Entity, Or<(Changed<LocalTransform>, Changed<ChildOf>, Added<GlobalTransform>)>>,
    mut orphaned: RemovedComponents<ChildOf>,
    mut transforms: Query<&mut TransformTreeChanged>,
    parents: Query<&ChildOf>,
    static_optimizations: Res<StaticTransformOptimizations>,
    // Cached allocations for multi-threaded parallel implementation
    mut shared_bitset: Local<
        Vec<AtomicU64>
    >,
    mut local_bitset: Local<
        Parallel<Vec<u64>>
    >,
    // mut consumer_channels: Local<
    //     Buffered
    // >
) {
    todo!()
}
