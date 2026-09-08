use std::{
    borrow::Borrow,
    fmt::Debug,
    hash::{BuildHasher, Hash},
    ops::{
        BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Deref, DerefMut, Sub,
        SubAssign,
    },
};

use hashbrown::{
    TryReserveError,
    hash_set::{Difference, Drain, ExtractIf, Intersection, Iter, SymmetricDifference, Union},
};
use rayon::iter::{FromParallelIterator, IntoParallelIterator, ParallelExtend};

use crate::hash::FixedHasher;

/// 使用 [`FixedHasher`] 的新 [`HashSet`]
#[repr(transparent)]
pub struct FixedHashSet<T, S = FixedHasher>(hashbrown::HashSet<T, S>);

impl<T, S> Clone for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

impl<T, S> Debug for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <hashbrown::HashSet<T, S> as Debug>::fmt(&self.0, f)
    }
}

impl<T, S> Default for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: Default,
{
    #[inline]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T, S> PartialEq for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T, S> Eq for FixedHashSet<T, S> where hashbrown::HashSet<T, S>: Eq {}

impl<T, S, X> FromIterator<X> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: FromIterator<X>,
{
    #[inline]
    fn from_iter<U: IntoIterator<Item = X>>(iter: U) -> Self {
        Self(FromIterator::from_iter(iter))
    }
}

impl<T, S> IntoIterator for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: IntoIterator,
{
    type Item = <hashbrown::HashSet<T, S> as IntoIterator>::Item;

    type IntoIter = <hashbrown::HashSet<T, S> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T, S> IntoIterator for &'a FixedHashSet<T, S>
where
    &'a hashbrown::HashSet<T, S>: IntoIterator,
{
    type Item = <&'a hashbrown::HashSet<T, S> as IntoIterator>::Item;

    type IntoIter = <&'a hashbrown::HashSet<T, S> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter()
    }
}

impl<'a, T, S> IntoIterator for &'a mut FixedHashSet<T, S>
where
    &'a mut hashbrown::HashSet<T, S>: IntoIterator,
{
    type Item = <&'a mut hashbrown::HashSet<T, S> as IntoIterator>::Item;

    type IntoIter = <&'a mut hashbrown::HashSet<T, S> as IntoIterator>::IntoIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        (&mut self.0).into_iter()
    }
}

impl<T, S, X> Extend<X> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: Extend<X>,
{
    #[inline]
    fn extend<U: IntoIterator<Item = X>>(&mut self, iter: U) {
        self.0.extend(iter);
    }
}

impl<T, const N: usize> From<[T; N]> for FixedHashSet<T, FixedHasher>
where
    T: Eq + Hash,
{
    fn from(value: [T; N]) -> Self {
        value.into_iter().collect()
    }
}

impl<T, S> From<hashbrown::HashSet<T, S>> for FixedHashSet<T, S> {
    #[inline]
    fn from(value: hashbrown::HashSet<T, S>) -> Self {
        Self(value)
    }
}

impl<T, S> From<FixedHashSet<T, S>> for hashbrown::HashSet<T, S> {
    #[inline]
    fn from(value: FixedHashSet<T, S>) -> Self {
        value.0
    }
}

impl<T, S> Deref for FixedHashSet<T, S> {
    type Target = hashbrown::HashSet<T, S>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, S> DerefMut for FixedHashSet<T, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, S> serde::Serialize for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: serde::Serialize,
{
    #[inline]
    fn serialize<U>(&self, serializer: U) -> Result<U::Ok, U::Error>
    where
        U: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T, S> serde::Deserialize<'de> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: serde::Deserialize<'de>,
{
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self(serde::Deserialize::deserialize(deserializer)?))
    }
}

impl<T> FixedHashSet<T, FixedHasher> {
    #[inline]
    pub const fn new() -> Self {
        Self::with_hasher(FixedHasher)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, FixedHasher)
    }
}

impl<T, S> FixedHashSet<T, S> {
    #[inline]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        self.0.iter()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[inline]
    pub fn drain(&mut self) -> Drain<'_, T> {
        self.0.drain()
    }

    #[inline]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.0.retain(f);
    }

    #[inline]
    pub fn extract_if<F>(&mut self, f: F) -> ExtractIf<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        self.0.extract_if(f)
    }

    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[inline]
    pub const fn with_hasher(hasher: S) -> Self {
        Self(hashbrown::HashSet::with_hasher(hasher))
    }

    #[inline]
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        Self(hashbrown::HashSet::with_capacity_and_hasher(
            capacity, hasher,
        ))
    }

    #[inline]
    pub fn hasher(&self) -> &S {
        self.0.hasher()
    }

    #[inline]
    pub fn into_inner(self) -> hashbrown::HashSet<T, S> {
        self.0
    }
}

impl<T, S> FixedHashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher,
{
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }

    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.0.shrink_to(min_capacity);
    }

    #[inline]
    pub fn difference<'a>(&'a self, other: &'a Self) -> Difference<'a, T, S> {
        self.0.difference(other)
    }

    #[inline]
    pub fn symmetric_difference<'a>(&'a self, other: &'a Self) -> SymmetricDifference<'a, T, S> {
        self.0.symmetric_difference(other)
    }

    #[inline]
    pub fn intersection<'a>(&'a self, other: &'a Self) -> Intersection<'a, T, S> {
        self.0.intersection(other)
    }

    #[inline]
    pub fn union<'a>(&'a self, other: &'a Self) -> Union<'a, T, S> {
        self.0.union(other)
    }

    #[inline]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.contains(value)
    }

    #[inline]
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.get(value)
    }

    #[inline]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(other)
    }

    #[inline]
    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(other)
    }

    #[inline]
    pub fn is_superset(&self, other: &Self) -> bool {
        self.0.is_superset(other)
    }

    #[inline]
    pub fn insert(&mut self, value: T) -> bool {
        self.0.insert(value)
    }

    #[inline]
    pub fn replace(&mut self, value: T) -> Option<T> {
        self.0.replace(value)
    }

    #[inline]
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.remove(value)
    }

    #[inline]
    pub fn take<Q>(&mut self, value: &Q) -> Option<T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.0.take(value)
    }

    /// Returns the total amount of memory allocated internally by the hash
    /// set, in bytes.
    ///
    /// Refer to [`allocation_size`](hb::HashSet::allocation_size) for further details.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use bevy_platform::collections::HashSet;
    /// let mut map = HashSet::new();
    ///
    /// assert_eq!(map.allocation_size(), 0);
    ///
    /// map.insert("foo");
    ///
    /// assert!(map.allocation_size() >= size_of::<&'static str>());
    /// ```
    #[inline]
    pub fn allocation_size(&self) -> usize {
        self.0.allocation_size()
    }

    /// Insert a value the set without checking if the value already exists in the set.
    ///
    /// Refer to [`insert_unique_unchecked`](hb::HashSet::insert_unique_unchecked) for further details.
    ///
    /// # Safety
    ///
    /// This operation is safe if a value does not exist in the set.
    ///
    /// However, if a value exists in the set already, the behavior is unspecified:
    /// this operation may panic, loop forever, or any following operation with the set
    /// may panic, loop forever or return arbitrary result.
    ///
    /// That said, this operation (and following operations) are guaranteed to
    /// not violate memory safety.
    ///
    /// However this operation is still unsafe because the resulting `HashSet`
    /// may be passed to unsafe code which does expect the set to behave
    /// correctly, and would cause unsoundness as a result.
    #[expect(
        unsafe_code,
        reason = "re-exporting unsafe method from Hashbrown requires unsafe code"
    )]
    #[inline]
    pub unsafe fn insert_unique_unchecked(&mut self, value: T) -> &T {
        // SAFETY: safety contract is ensured by the caller.
        unsafe { self.0.insert_unique_unchecked(value) }
    }
}

impl<T, S> BitOr<&FixedHashSet<T, S>> for &FixedHashSet<T, S>
where
    for<'a> &'a hashbrown::HashSet<T, S>:
        BitOr<&'a hashbrown::HashSet<T, S>, Output = hashbrown::HashSet<T, S>>,
{
    type Output = FixedHashSet<T, S>;

    #[inline]
    fn bitor(self, rhs: &FixedHashSet<T, S>) -> Self::Output {
        FixedHashSet(self.0.bitor(&rhs.0))
    }
}

impl<T, S> BitAnd<&FixedHashSet<T, S>> for &FixedHashSet<T, S>
where
    for<'a> &'a hashbrown::HashSet<T, S>:
        BitAnd<&'a hashbrown::HashSet<T, S>, Output = hashbrown::HashSet<T, S>>,
{
    type Output = FixedHashSet<T, S>;

    #[inline]
    fn bitand(self, rhs: &FixedHashSet<T, S>) -> Self::Output {
        FixedHashSet(self.0.bitand(&rhs.0))
    }
}

impl<T, S> BitXor<&FixedHashSet<T, S>> for &FixedHashSet<T, S>
where
    for<'a> &'a hashbrown::HashSet<T, S>:
        BitXor<&'a hashbrown::HashSet<T, S>, Output = hashbrown::HashSet<T, S>>,
{
    type Output = FixedHashSet<T, S>;

    #[inline]
    fn bitxor(self, rhs: &FixedHashSet<T, S>) -> Self::Output {
        FixedHashSet(self.0.bitxor(&rhs.0))
    }
}

impl<T, S> Sub<&FixedHashSet<T, S>> for &FixedHashSet<T, S>
where
    for<'a> &'a hashbrown::HashSet<T, S>:
        Sub<&'a hashbrown::HashSet<T, S>, Output = hashbrown::HashSet<T, S>>,
{
    type Output = FixedHashSet<T, S>;

    #[inline]
    fn sub(self, rhs: &FixedHashSet<T, S>) -> Self::Output {
        FixedHashSet(self.0.sub(&rhs.0))
    }
}

impl<T, S> BitOrAssign<&FixedHashSet<T, S>> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: for<'a> BitOrAssign<&'a hashbrown::HashSet<T, S>>,
{
    /// Modifies this set to contain the union of `self` and `rhs`.
    #[inline]
    fn bitor_assign(&mut self, rhs: &FixedHashSet<T, S>) {
        self.0.bitor_assign(&rhs.0);
    }
}

impl<T, S> BitAndAssign<&FixedHashSet<T, S>> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: for<'a> BitAndAssign<&'a hashbrown::HashSet<T, S>>,
{
    /// Modifies this set to contain the intersection of `self` and `rhs`.
    #[inline]
    fn bitand_assign(&mut self, rhs: &FixedHashSet<T, S>) {
        self.0.bitand_assign(&rhs.0);
    }
}

impl<T, S> BitXorAssign<&FixedHashSet<T, S>> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: for<'a> BitXorAssign<&'a hashbrown::HashSet<T, S>>,
{
    /// Modifies this set to contain the symmetric difference of `self` and `rhs`.
    #[inline]
    fn bitxor_assign(&mut self, rhs: &FixedHashSet<T, S>) {
        self.0.bitxor_assign(&rhs.0);
    }
}

impl<T, S> SubAssign<&FixedHashSet<T, S>> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: for<'a> SubAssign<&'a hashbrown::HashSet<T, S>>,
{
    /// Modifies this set to contain the difference of `self` and `rhs`.
    #[inline]
    fn sub_assign(&mut self, rhs: &FixedHashSet<T, S>) {
        self.0.sub_assign(&rhs.0);
    }
}

impl<T, S, U> FromParallelIterator<U> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: FromParallelIterator<U>,
    U: Send,
{
    fn from_par_iter<P>(par_iter: P) -> Self
    where
        P: IntoParallelIterator<Item = U>,
    {
        Self(<hashbrown::HashSet<T, S> as FromParallelIterator<U>>::from_par_iter(par_iter))
    }
}

impl<T, S> IntoParallelIterator for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: IntoParallelIterator,
{
    type Item = <hashbrown::HashSet<T, S> as IntoParallelIterator>::Item;
    type Iter = <hashbrown::HashSet<T, S> as IntoParallelIterator>::Iter;

    fn into_par_iter(self) -> Self::Iter {
        self.0.into_par_iter()
    }
}

impl<'a, T: Sync, S> IntoParallelIterator for &'a FixedHashSet<T, S>
where
    &'a hashbrown::HashSet<T, S>: IntoParallelIterator,
{
    type Item = <&'a hashbrown::HashSet<T, S> as IntoParallelIterator>::Item;
    type Iter = <&'a hashbrown::HashSet<T, S> as IntoParallelIterator>::Iter;

    fn into_par_iter(self) -> Self::Iter {
        (&self.0).into_par_iter()
    }
}

impl<T, S, U> ParallelExtend<U> for FixedHashSet<T, S>
where
    hashbrown::HashSet<T, S>: ParallelExtend<U>,
    U: Send,
{
    fn par_extend<I>(&mut self, par_iter: I)
    where
        I: IntoParallelIterator<Item = U>,
    {
        <hashbrown::HashSet<T, S> as ParallelExtend<U>>::par_extend(&mut self.0, par_iter);
    }
}
