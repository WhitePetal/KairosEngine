//! Logic for evaluating observers, and storing functions inside of observers.

use crate::{
    ecs::{entity::Entity, observer::TriggerContext, world::DeferredWorld},
    ptr::PtrMut,
};

pub type ObserverRunner =
    unsafe fn(DeferredWorld, observer: Entity, &TriggerContext, event: PtrMut, trigger: PtrMut);

// TODO!
