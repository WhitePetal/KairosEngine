//! System parameters for working with observers.

use crate::{debug::MaybeLocation, ecs::event::EventKey};

/// Metadata about a specific [`Event`] that triggered an observer.
///
/// This information is exposed via methods on [`On`].
pub struct TriggerContext {
    /// The [`EventKey`] the trigger targeted.
    pub event_key: EventKey,
    /// The location of the source code that triggered the observer.
    pub caller: MaybeLocation,
}

// TODO!
