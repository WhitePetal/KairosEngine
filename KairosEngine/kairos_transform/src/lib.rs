//! Local/Global transform ECS components for the Kairos engine.
//!
//! Extracted from `kairos_engine::spatial::transform` as a standalone crate
//! (bevy `bevy_transform` parity), built on the `kairos_math` primitives:
//!
//! - [`LocalTransform`] — an entity's own transform relative to its parent
//!   (root-level entities: relative to the world). All three fields are
//!   public: `position` / `rotation` / `scale`.
//! - [`GlobalTransform`] — an entity's absolute (world-space) transform,
//!   stored as a [`kairos_math::affine`].
//!
//! Coordinate system: right-handed, **Y-up, -Z forward** (the engine's spatial
//! convention).
//!
//! # Component lifecycle
//!
//! The propagation system that maintains each entity's [`GlobalTransform`]
//! from its [`LocalTransform`] hierarchy has **not** landed yet (a wayfinder
//! map follow-up). Until it does:
//!
//! - do not spawn entities with a [`GlobalTransform`], and do not read it at
//!   consumption sites — while every scene is a root scene (`local == world`),
//!   read [`LocalTransform`] directly;
//! - [`GlobalTransform`] is still a spawnable `Component` with a meaningful
//!   [`Default`] (the identity transform) so the API is in place for the
//!   propagation system.

mod global_transform;
mod local_transform;

pub use global_transform::GlobalTransform;
pub use local_transform::LocalTransform;
