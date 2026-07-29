use crate::ecs::entity::EntityEquivalent;


/// An array that contains only unique entities.
///
/// It can be obtained through certain methods on [`UniqueEntityEquivalentSlice`],
/// and some [`TryFrom`] implementations.
///
/// When `T` is [`Entity`], use [`UniqueEntityArray`].
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueEntityEquivalentArray<T: EntityEquivalent, const N: usize>([T; N]);


// TODO!
