use crate::ecs::event::Event;

pub unsafe trait Trigger<E: Event> {}

/// A [`Trigger`] that runs _every_ "global" [`Observer`](crate::observer::Observer) (ex: registered via [`World::add_observer`](crate::world::World::add_observer))
/// that matches the given [`Event`].
///
/// The [`Event`] derive defaults to using this [`Trigger`], and it is usable for any [`Event`] type.
#[derive(Default, Debug)]
pub struct GlobalTrigger;

unsafe impl<E: for<'a> Event<Trigger<'a> = Self>> Trigger<E> for GlobalTrigger {}
