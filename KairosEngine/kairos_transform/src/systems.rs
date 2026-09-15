use std::sync::atomic::{AtomicU64, Ordering};

use kairos_ecs::{change_detection::{DetectChanges, DetectChangesMut, Ref}, entity::Entity, hierarchy::{ChildOf, Children}, lifecycle::RemovedComponents, query::{Added, Changed, Or, QueryFilter, Without}, resource::Resource, system::{Local, ParamSet, Query, Res, lifetimeless::SQuery}};
use kairos_tasks::{ComputeTaskPool, buffered_channel::BufferedChannel, parallel_queue::Parallel};
#[cfg(feature = "trace")]
use tracing::{Instrument, info_span};

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
        SQuery<
            (&LocalTransform, &mut GlobalTransform),
            (
                Or<(Changed<LocalTransform>, Added<GlobalTransform>)>,
                Without<ChildOf>,
                Without<Children>
            )
        >,
        SQuery<
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

/// Optimization for static scenes.
///
/// Propagates a "dirty bit" up the hierarchy towards ancestors. Transform propagation can ignore
/// entire subtrees of the hierarchy if it encounters an entity without the dirty bit.
///
/// Configure behavior with [`StaticTransformOptimizations`].
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
    mut consumer_channels: Local<
        BufferedChannel<Entity>,
    >,
    mut traversal_channels: Local<
        BufferedChannel<Entity>,
    >,
) {
    if !static_optimizations.is_enabled() {
        return;
    }

    ComputeTaskPool::get().scope(|scope| {
        traversal_channels.chunk_size = 1024;
        consumer_channels.chunk_size = 1024;
        let (traversal_rx, mut traversal_tx) = traversal_channels.unbounded();
        let (consumer_rx, mut consumer_tx) = consumer_channels.unbounded();
        let shared_bitset: &[AtomicU64] = &shared_bitset;
        let local_bitset = &*local_bitset;
        let parents_ref = &parents;

        // Consumer: drain the channel of moved entities and call set_changed() on the marker.
        scope.spawn({
            let fut = async move {
                while let Ok(mut chunk) = consumer_rx.recv().await {
                    for entity in chunk.drain() {
                        if let Ok(mut tree) = transforms.get_mut(entity) {
                            tree.set_changed();
                        }
                    }
                }
            };
            #[cfg(feature = "trace")]
            let fut = fut.instrument(info_span!("consumer_mark_dirty"));
            fut
        });

        // Traversal: each task loops until the producer channel is exhausted, walking each
        // entity's ancestor chain and forwarding newly marked entities to the consumer task.
        for _ in 0..(ComputeTaskPool::get().thread_num() - 1).max(1) {
            let traversal_rx = traversal_rx.clone();
            let mut consumer_tx = consumer_tx.clone();
            scope.spawn({
                let fut = async move {
                    while let Ok(mut chunk) = traversal_rx.recv().await {
                        for mut entity in chunk.drain() {
                            let mut first_iteration = true;
                            'traverse_hierarchy: loop {
                                let idx = entity.index().index() as usize;
                                let word = idx / 64;
                                let bit = 1u64 << (idx % 64);

                                #[expect(
                                    clippy::redundant_else,
                                    reason = "Without the else, fails to compile due to async"
                                )]
                                if word < shared_bitset.len()
                                    && shared_bitset[word].fetch_or(bit, Ordering::Relaxed)
                                        & bit
                                        != 0
                                {
                                    // Common path: atomic OR into the shared bitset.
                                    // If the entity was already visited, we can stop climbing.
                                    break 'traverse_hierarchy;
                                } else {
                                    // Overflow: entity index exceeds shared bitset capacity.
                                    // Use a per-task local bitset for intra-task early exit.
                                    let overflow = &mut *local_bitset.borrow_local_mut();
                                    if word < overflow.len() && overflow[word] & bit != 0 {
                                        break 'traverse_hierarchy;
                                    }
                                    if word >= overflow.len() {
                                        overflow.resize(word + 1, 0u64);
                                    }
                                    overflow[word] |= bit;
                                }

                                // If we have not hit a break yet, it's the first time we've
                                // seen this entity, so it should be sent to the consumer.
                                if first_iteration {
                                    first_iteration = false;
                                } else {
                                    // The first iteration (leaf) has already been sent to the
                                    // consumer by the producer; we don't need to send it again.
                                    consumer_tx.send(entity).await.ok();
                                }

                                match parents_ref.get(entity).ok().map(ChildOf::parent) {
                                    Some(parent) => entity = parent,
                                    None => break 'traverse_hierarchy,
                                }
                            }
                        }
                    }
                };
                #[cfg(feature = "trace")]
                let fut = fut.instrument(info_span!("par_traversal_mark_dirty"));
                fut
            });
        }

        // Producer: Feed changed entities and orphans into producer tasks. The senders are
        // dropped at the end of this closure, closing the channel and allowing the other tasks
        // to exit.
        //
        // Note that we send the entity directly to the consumer as well, we do this to start
        // feeding it work as soon as possible. The traversal worker should skip sending these
        // leaves to the consumer because it has already been sent here.
        let mut producer = move || {
            for entity in orphaned.read() {
                let _ = traversal_tx.send_blocking(entity);
                let _ = consumer_tx.send_blocking(entity);
            }
            // Changed<> table scans are slow, so we parallelize them to improve performance.
            changed.par_iter().for_each_init(
                || (traversal_tx.clone(), consumer_tx.clone()),
                |(traversal_tx, consumer_tx), entity| {
                    let _ = traversal_tx.send_blocking(entity);
                    let _ = consumer_tx.send_blocking(entity);
                },
            );
        };
        #[cfg(feature = "trace")]
        info_span!("producer_mark_dirty").in_scope(&mut producer);
        #[cfg(not(feature = "trace"))]
        producer()
    });

    // Merge thread-local bitsets into the shared bitset, growing it to accommodate the largest
    // entity index we have encountered so far. At steady-state, these local bitsets stay empty.
    for local_bitset in local_bitset.iter_mut() {
        if local_bitset.is_empty() {
            continue;
        }
        if local_bitset.len() > shared_bitset.len() {
            shared_bitset.resize_with(local_bitset.len(), Default::default);
        }
        local_bitset.clear();
    }

    // Reset the bitset for the next frame while preserving the `Vec` length. Using `clear()`
    // would shrink the length to 0 and force every entity through the overflow path next frame.
    for w in shared_bitset.iter() {
        w.store(0, Ordering::Relaxed);
    }
}
