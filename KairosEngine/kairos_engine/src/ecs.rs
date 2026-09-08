pub mod archetype;
pub mod batching;
pub mod bundle;
pub mod change_detection;
pub mod component;
pub mod entity;
pub mod entity_disabling;
pub mod error;
pub mod event;
pub mod hierarchy;
pub mod intern;
pub mod label;
pub mod lifecycle;
pub mod message;
pub mod name;
pub mod never;
pub mod observer;
pub mod query;
#[cfg(feature = "kairos_reflect")]
pub mod reflect;
pub mod relationship;
pub mod resource;
pub mod schedule;
pub mod spawn;
pub mod storage;
pub mod system;
pub mod template;
pub mod traversal;
pub mod world;

/// The ECS prelude.
///
/// This includes the most common types in this crate, re-exported for your convenience.
pub mod prelude {
    #[doc(hidden)]
    pub use crate::ecs::{
        bundle::Bundle,
        change_detection::{
            ContiguousMut, ContiguousRef, DetectChanges, DetectChangesMut, Mut, Ref,
        },
        component::Component,
        entity::{ContainsEntity, Entity, EntityMapper},
        error::{KairosError, Result, ResultSeverityExt, Severity},
        event::{EntityEvent, Event},
        hierarchy::{ChildOf, ChildSpawner, ChildSpawnerCommands, Children},
        lifecycle::{Add, Despawn, Discard, Insert, Remove, RemovedComponents},
        message::{
            Message, MessageMutator, MessageReader, MessageWriter, Messages, PopulatedMessageReader,
        },
        name::{Name, NameOrEntity},
        observer::{Observer, ObserverSystemExt, On},
        query::{Added, Allow, AnyOf, Changed, Has, Or, QueryBuilder, QueryState, With, Without},
        relationship::RelationshipTarget,
        resource::Resource,
        schedule::{
            ApplyDeferred, IntoScheduleConfigs, IntoSystemSet, Schedule, Schedules,
            SystemCondition, SystemSet, common_conditions::*,
        },
        spawn::{Spawn, SpawnIter, SpawnRelated, SpawnWith, WithOneRelated, WithRelated},
        system::{
            Command, Commands, Deferred, EntityCommand, EntityCommands, If, In, InMut, InRef,
            IntoSystem, Local, NonSend, NonSendMut, ParamSet, Populated, Query, ReadOnlySystem,
            Res, ResMut, Single, System, SystemIn, SystemInput, SystemParamBuilder,
            SystemParamFunction,
        },
        template::{FromTemplate, Template, template},
        world::{
            EntityMut, EntityRef, EntityWorldMut, FilteredResources, FilteredResourcesMut,
            FromWorld, World,
        },
    };
    pub use crate::{children, related};

    #[doc(hidden)]
    pub use crate::ecs::system::ParallelCommands;

    #[doc(hidden)]
    #[cfg(feature = "kairos_reflect")]
    pub use crate::reflect::{
        AppTypeRegistry, ReflectComponent, ReflectEvent, ReflectFromWorld, ReflectMessage,
        ReflectResource,
    };

    // #[doc(hidden)]
    // #[cfg(feature = "reflect_functions")]
    // pub use crate::reflect::AppFunctionRegistry;
}

/// Exports used by macros.
///
/// These are not meant to be used directly and are subject to breaking changes.
#[doc(hidden)]
pub mod __macro_exports {
    // Cannot directly use `alloc::vec::Vec` in macros, as a crate may not have
    // included `extern crate alloc;`. This re-export ensures we have access
    // to `Vec` in `no_std` and `std` contexts.
    pub use crate::debug::DebugCheckedUnwrap;
    pub use crate::ptr::{MovingPtr, OwningPtr, deconstruct_moving_ptr};
}
