use std::vec;

use crate::ecs::entity::{Entity, EntityEquivalent, UniqueEntityIter};



/// A `Vec` that contains only unique entities.
///
/// "Unique" means that `x != y` holds for any 2 entities in this collection.
/// This is always true when less than 2 entities are present.
///
/// This type is best obtained by its `FromEntitySetIterator` impl, via either
/// `EntityIterator::collect_set` or `UniqueEntityEquivalentVec::from_entity_iter`.
///
/// While this type can be constructed via `Iterator::collect`, doing so is inefficient,
/// and not recommended.
///
/// When `T` is [`Entity`], use the [`UniqueEntityVec`] alias.
#[repr(transparent)]
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueEntityEquivalentVec<T: EntityEquivalent>(Vec<T>);

/// An iterator that moves out of a vector.
///
/// This `struct` is created by the [`IntoIterator::into_iter`] trait
/// method on [`UniqueEntityEquivalentVec`].
pub type IntoIter<T = Entity> = UniqueEntityIter<vec::IntoIter<T>>;

//TODO!
