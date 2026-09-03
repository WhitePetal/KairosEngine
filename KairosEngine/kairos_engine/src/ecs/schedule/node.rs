use std::{
    any::TypeId,
    fmt::{self, Debug},
};

use slotmap::{Key, KeyData, SecondaryMap, SlotMap, new_key_type};

use crate::{
    debug::DebugName,
    ecs::{
        query::FilteredAccessSet,
        schedule::{
            BoxedCondition,
            graph::{Direction, GraphNodeId},
        },
        system::{RunSystemError, ScheduleSystem, System, SystemIn, SystemStateFlags},
        world::{DeferredWorld, World, unsafe_world_cell::UnsafeWorldCell},
    },
};

/// A [`SystemWithAccess`] stored in a [`ScheduleGraph`].
pub(crate) struct SystemNode {
    pub(crate) inner: Option<SystemWithAccess>,
}

/// A [`ScheduleSystem`] stored alongside the access returned from [`System::initialize`].s
pub struct SystemWithAccess {
    /// The system itself.
    pub(crate) system: ScheduleSystem,
    /// The access returned by [`System::initialize`].
    /// This will be empty if the system has not been initialized yet.
    pub(crate) access: FilteredAccessSet,
}

impl SystemWithAccess {
    /// Constructs a new [`SystemWithAccess`] from a [`ScheduleSystem`].
    /// The `access` will initially be empty.
    pub fn new(system: ScheduleSystem) -> Self {
        Self {
            system,
            access: FilteredAccessSet::new(),
        }
    }

    /// Returns the underlying [`ScheduleSystem`]
    pub fn system(&self) -> &ScheduleSystem {
        &self.system
    }
}

impl System for SystemWithAccess {
    type In = ();

    type Out = ();

    #[inline]
    fn name(&self) -> DebugName {
        self.system.name()
    }

    #[inline]
    fn system_type(&self) -> TypeId {
        self.system.system_type()
    }

    #[inline]
    fn flags(&self) -> SystemStateFlags {
        self.system.flags()
    }

    #[inline]
    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        // SAFETY: Caller ensures the same safety requirements.
        unsafe { self.system.run_unsafe(input, world) }
    }

    #[cfg(feature = "hotpatching")]
    #[inline]
    fn refresh_hotpatch(&mut self) {
        self.system.refresh_hotpatch();
    }

    #[inline]
    fn apply_deferred(&mut self, world: &mut World) {
        self.system.apply_deferred(world);
    }

    #[inline]
    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.system.queue_deferred(world);
    }

    #[inline]
    fn initialize(&mut self, world: &mut crate::ecs::world::World) -> FilteredAccessSet {
        self.system.initialize(world)
    }

    #[inline]
    fn check_change_tick(&mut self, check: crate::ecs::change_detection::CheckChangeTicks) {
        self.system.check_change_tick(check);
    }

    #[inline]
    fn default_system_sets(&self) -> Vec<super::InternedSystemSet> {
        self.system.default_system_sets()
    }

    #[inline]
    fn get_last_run(&self) -> crate::ecs::change_detection::Tick {
        self.system.get_last_run()
    }

    #[inline]
    fn set_last_run(&mut self, last_run: crate::ecs::change_detection::Tick) {
        self.system.set_last_run(last_run);
    }
}

/// A [`BoxedCondition`] stored alongside the access returned from [`System::initialize`].
pub struct ConditionWithAccess {
    /// The condition itself.
    pub condition: BoxedCondition,
    /// The access returned by [`System::initialize`].
    /// This will be empty if the system has not been initialized yet.
    pub access: FilteredAccessSet,
}

impl ConditionWithAccess {
    /// Constructs a new [`ConditionWithAccess`] from a [`BoxedCondition`].
    /// The `access` will initially be empty.
    pub const fn new(condition: BoxedCondition) -> Self {
        Self {
            condition,
            access: FilteredAccessSet::new(),
        }
    }
}

impl System for ConditionWithAccess {
    type In = ();

    type Out = bool;

    #[inline]
    fn name(&self) -> DebugName {
        self.condition.name()
    }

    #[inline]
    fn system_type(&self) -> TypeId {
        self.condition.system_type()
    }

    #[inline]
    fn flags(&self) -> SystemStateFlags {
        self.condition.flags()
    }

    #[inline]
    unsafe fn run_unsafe(
        &mut self,
        input: SystemIn<'_, Self>,
        world: UnsafeWorldCell,
    ) -> Result<Self::Out, RunSystemError> {
        // SAFETY: Caller ensures the same safety requirements.
        unsafe { self.condition.run_unsafe(input, world) }
    }

    #[cfg(feature = "hotpatching")]
    #[inline]
    fn refresh_hotpatch(&mut self) {
        self.condition.refresh_hotpatch();
    }

    #[inline]
    fn apply_deferred(&mut self, world: &mut World) {
        self.condition.apply_deferred(world);
    }

    #[inline]
    fn queue_deferred(&mut self, world: DeferredWorld) {
        self.condition.queue_deferred(world);
    }

    #[inline]
    fn initialize(&mut self, world: &mut World) -> FilteredAccessSet {
        self.condition.initialize(world)
    }

    #[inline]
    fn check_change_tick(&mut self, check: crate::ecs::change_detection::CheckChangeTicks) {
        self.condition.check_change_tick(check);
    }

    #[inline]
    fn default_system_sets(&self) -> Vec<super::InternedSystemSet> {
        self.condition.default_system_sets()
    }

    #[inline]
    fn get_last_run(&self) -> crate::ecs::change_detection::Tick {
        self.condition.get_last_run()
    }

    #[inline]
    fn set_last_run(&mut self, last_run: crate::ecs::change_detection::Tick) {
        self.condition.set_last_run(last_run)
    }
}

impl SystemNode {
    /// Create a new [`SystemNode`]
    pub fn new(system: ScheduleSystem) -> Self {
        Self {
            inner: Some(SystemWithAccess::new(system)),
        }
    }

    /// Obtain a reference to the [`SystemWithAccess`] represented by this node.
    pub fn get(&self) -> Option<&SystemWithAccess> {
        self.inner.as_ref()
    }

    /// Obtain a mutable reference to the [`SystemWithAccess`] represented by this node.
    pub fn get_mut(&mut self) -> Option<&mut SystemWithAccess> {
        self.inner.as_mut()
    }
}

new_key_type! {
    /// A unique identifier for a system in a [`ScheduleGraph`].
    pub struct SystemKey;
    /// A unique identifier for a system set in a [`ScheduleGraph`].
    pub struct SystemSetKey;
}

impl GraphNodeId for SystemKey {
    type Adjacent = (SystemKey, Direction);
    type Edge = (SystemKey, SystemKey);

    fn kind(&self) -> &'static str {
        "system"
    }
}

/// Container for systems in a schedule.
#[derive(Default)]
pub struct Systems {
    nodes: SlotMap<SystemKey, SystemNode>,
    conditions: SecondaryMap<SystemKey, Vec<ConditionWithAccess>>,
    uninit: Vec<SystemKey>,
}

/// Unique identifier for a system or system set stored in a [`ScheduleGraph`].
///
/// [`ScheduleGraph`]: crate::schedule::ScheduleGraph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeId {
    /// Identifier for a system.
    System(SystemKey),
    /// Identifier for a system set.
    Set(SystemSetKey),
}

impl NodeId {
    /// Returns `true` if the identified node is a system.
    pub const fn is_system(&self) -> bool {
        matches!(self, NodeId::System(_))
    }
}

impl GraphNodeId for NodeId {
    type Adjacent = CompactNodeIdAndDirection;
    type Edge = CompactNodeIdPair;

    fn kind(&self) -> &'static str {
        match self {
            NodeId::System(n) => n.kind(),
            NodeId::Set(n) => n.kind(),
        }
    }
}

impl GraphNodeId for SystemSetKey {
    type Adjacent = (SystemSetKey, Direction);
    type Edge = (SystemSetKey, SystemSetKey);

    fn kind(&self) -> &'static str {
        "system set"
    }
}

/// Compact storage of a [`NodeId`] and a [`Direction`].
#[derive(Clone, Copy)]
pub struct CompactNodeIdAndDirection {
    key: KeyData,
    is_system: bool,
    direction: Direction,
}

impl Debug for CompactNodeIdAndDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tuple: (_, _) = (*self).into();
        tuple.fmt(f)
    }
}

impl From<(NodeId, Direction)> for CompactNodeIdAndDirection {
    fn from((id, direction): (NodeId, Direction)) -> Self {
        let key = match id {
            NodeId::System(key) => key.data(),
            NodeId::Set(key) => key.data(),
        };
        let is_system = id.is_system();

        Self {
            key,
            is_system,
            direction,
        }
    }
}

impl From<CompactNodeIdAndDirection> for (NodeId, Direction) {
    fn from(value: CompactNodeIdAndDirection) -> Self {
        let node = match value.is_system {
            true => NodeId::System(value.key.into()),
            false => NodeId::Set(value.key.into()),
        };

        (node, value.direction)
    }
}

/// Compact storage of a [`NodeId`] pair.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct CompactNodeIdPair {
    key_a: KeyData,
    key_b: KeyData,
    is_system_a: bool,
    is_system_b: bool,
}

impl Debug for CompactNodeIdPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tuple: (_, _) = (*self).into();
        tuple.fmt(f)
    }
}

impl From<(NodeId, NodeId)> for CompactNodeIdPair {
    fn from((a, b): (NodeId, NodeId)) -> Self {
        let key_a = match a {
            NodeId::System(index) => index.data(),
            NodeId::Set(index) => index.data(),
        };
        let is_system_a = a.is_system();

        let key_b = match b {
            NodeId::System(index) => index.data(),
            NodeId::Set(index) => index.data(),
        };
        let is_system_b = b.is_system();

        Self {
            key_a,
            key_b,
            is_system_a,
            is_system_b,
        }
    }
}

impl From<CompactNodeIdPair> for (NodeId, NodeId) {
    fn from(value: CompactNodeIdPair) -> Self {
        let a = match value.is_system_a {
            true => NodeId::System(value.key_a.into()),
            false => NodeId::Set(value.key_a.into()),
        };

        let b = match value.is_system_b {
            true => NodeId::System(value.key_b.into()),
            false => NodeId::Set(value.key_b.into()),
        };

        (a, b)
    }
}
