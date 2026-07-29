use std::{borrow::Borrow, cmp::Ordering, ops::{Deref, Index, Range}, ptr, rc::Rc, slice::{self, SliceIndex}, sync::Arc};

use crate::ecs::entity::{Entity, EntityEquivalent, UniqueEntityIter, unique_array::UniqueEntityEquivalentArray, unique_vec::{self, UniqueEntityEquivalentVec}};



/// A slice that contains only unique entities.
///
/// This can be obtained by slicing [`UniqueEntityEquivalentVec`].
///
/// When `T` is [`Entity`], use [`UniqueEntitySlice`].
#[repr(transparent)]
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueEntityEquivalentSlice<T: EntityEquivalent>([T]);


/// A slice that contains only unique [`Entity`].
///
/// This is the default case of a [`UniqueEntityEquivalentSlice`].
pub type UniqueEntitySlice = UniqueEntityEquivalentSlice<Entity>;

/// Immutable slice iterator.
///
/// This struct is created by [`iter`] method on [`UniqueEntityEquivalentSlice`] and
/// the [`IntoIterator`] impls on it and [`UniqueEntityEquivalentVec`].
///
/// [`iter`]: `UniqueEntityEquivalentSlice::iter`
pub type Iter<'a, T> = UniqueEntityIter<slice::Iter<'a, T>>;

/// An iterator over overlapping subslices of length `size`.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::windows`].
pub type Windows<'a, T = Entity> = UniqueEntityEquivalentSliceIter<'a, T, slice::Windows<'a, T>>;

/// An iterator over a slice in (non-overlapping) chunks (`chunk_size` elements at a
/// time), starting at the beginning of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::chunks`].
pub type Chunks<'a, T = Entity> = UniqueEntityEquivalentSliceIter<'a, T, slice::Chunks<'a, T>>;

/// An iterator over a slice in (non-overlapping) chunks (`chunk_size` elements at a
/// time), starting at the beginning of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::chunks_exact`].
pub type ChunksExact<'a, T = Entity> =
    UniqueEntityEquivalentSliceIter<'a, T, slice::ChunksExact<'a, T>>;

/// An iterator over a slice in (non-overlapping) mutable chunks (`chunk_size`
/// elements at a time), starting at the beginning of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::chunks_mut`].
pub type ChunksMut<'a, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::ChunksMut<'a, T>>;

/// An iterator over a slice in (non-overlapping) mutable chunks (`chunk_size`
/// elements at a time), starting at the beginning of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::chunks_exact_mut`].
pub type ChunksExactMut<'a, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::ChunksExactMut<'a, T>>;

/// An iterator over a slice in (non-overlapping) chunks (`chunk_size` elements at a
/// time), starting at the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rchunks`].
pub type RChunks<'a, T = Entity> = UniqueEntityEquivalentSliceIter<'a, T, slice::RChunks<'a, T>>;

/// An iterator over a slice in (non-overlapping) mutable chunks (`chunk_size`
/// elements at a time), starting at the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rchunks_mut`].
pub type RChunksMut<'a, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::RChunksMut<'a, T>>;

/// An iterator over a slice in (non-overlapping) chunks (`chunk_size` elements at a
/// time), starting at the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rchunks_exact`].
pub type RChunksExact<'a, T = Entity> =
    UniqueEntityEquivalentSliceIter<'a, T, slice::RChunksExact<'a, T>>;

/// An iterator over a slice in (non-overlapping) mutable chunks (`chunk_size`
/// elements at a time), starting at the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rchunks_exact_mut`].
pub type RChunksExactMut<'a, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::RChunksExactMut<'a, T>>;

/// An iterator over slice in (non-overlapping) chunks separated by a predicate.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::chunk_by`].
pub type ChunkBy<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIter<'a, T, slice::ChunkBy<'a, T, P>>;

/// An iterator over slice in (non-overlapping) mutable chunks separated
/// by a predicate.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::chunk_by_mut`].
pub type ChunkByMut<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::ChunkByMut<'a, T, P>>;

/// An iterator over the mutable subslices of the vector which are separated
/// by elements that match `pred`.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::split_mut`].
pub type SplitMut<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::SplitMut<'a, T, P>>;

/// An iterator over the mutable subslices of the vector which are separated
/// by elements that match `pred`. Unlike `SplitMut`, it contains the matched
/// parts in the ends of the subslices.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::split_inclusive_mut`].
pub type SplitInclusiveMut<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::SplitInclusiveMut<'a, T, P>>;

/// An iterator over subslices separated by elements that match a predicate
/// function.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::split`].
pub type Split<'a, P, T = Entity> = UniqueEntityEquivalentSliceIter<'a, T, slice::Split<'a, T, P>>;

/// An iterator over subslices separated by elements that match a predicate
/// function.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::split_inclusive`].
pub type SplitInclusive<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIter<'a, T, slice::SplitInclusive<'a, T, P>>;

/// An iterator over subslices separated by elements that match a predicate
/// function, starting from the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rsplit`].
pub type RSplit<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIter<'a, T, slice::RSplit<'a, T, P>>;

/// An iterator over the subslices of the vector which are separated
/// by elements that match `pred`, starting from the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rsplit_mut`].
pub type RSplitMut<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::RSplitMut<'a, T, P>>;

/// An iterator over subslices separated by elements that match a predicate
/// function, limited to a given number of splits.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::splitn`].
pub type SplitN<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIter<'a, T, slice::SplitN<'a, T, P>>;

/// An iterator over subslices separated by elements that match a predicate
/// function, limited to a given number of splits.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::splitn_mut`].
pub type SplitNMut<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::SplitNMut<'a, T, P>>;

/// An iterator over subslices separated by elements that match a
/// predicate function, limited to a given number of splits, starting
/// from the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rsplitn_mut`].
pub type RSplitNMut<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIterMut<'a, T, slice::RSplitNMut<'a, T, P>>;

/// An iterator over subslices separated by elements that match a
/// predicate function, limited to a given number of splits, starting
/// from the end of the slice.
///
/// This struct is created by [`UniqueEntityEquivalentSlice::rsplitn`].
pub type RSplitN<'a, P, T = Entity> =
    UniqueEntityEquivalentSliceIter<'a, T, slice::RSplitN<'a, T, P>>;

impl<T: EntityEquivalent> UniqueEntityEquivalentSlice<T> {
    /// Constructs a `UniqueEntityEquivalentSlice` from a [`&[T]`] unsafely.
    ///
    /// # Safety
    ///
    /// `slice` must contain only unique elements.
    pub const unsafe fn from_slice_unchecked(slice: &[T]) -> &Self {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { &*(ptr::from_ref(slice) as *const Self ) }
    }

    /// Constructs a `UniqueEntityEquivalentSlice` from a [`&mut [T]`] unsafely.
    ///
    /// # Safety
    ///
    /// `slice` must contain only unique elements.
    pub const unsafe fn from_slice_unchecked_mut(slice: &mut [T]) -> &mut Self {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { &mut *(ptr::from_mut(slice) as *mut Self) }
    }

    /// Casts to `self` to a standard slice.
    pub const fn as_inner(&self) -> &[T] {
        &self.0
    }

    /// Constructs a `UniqueEntityEquivalentSlice` from a [`Box<[T]>`] unsafely.
    ///
    /// # Safety
    ///
    /// `slice` must contain only unique elements.
    pub unsafe fn from_boxed_slice_unchecked(slice: Box<[T]>) -> Box<Self> {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { Box::from_raw(Box::into_raw(slice) as *mut Self) }
    }

    /// Casts `self` to the inner slice.
    pub fn into_boxed_inner(self: Box<Self>) -> Box<[T]> {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { Box::from_raw(Box::into_raw(self) as *mut [T]) }
    }

    /// Constructs a `UniqueEntityEquivalentSlice` from a [`Arc<[T]>`] unsafely.
    ///
    /// # Safety
    ///
    /// `slice` must contain only unique elements.
    pub unsafe fn from_arc_slice_unchecked(slice: Arc<[T]>) -> Arc<Self> {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { Arc::from_raw(Arc::into_raw(slice) as *mut Self) }
    }

    /// Casts `self` to the inner slice.
    pub fn into_arc_inner(this: Arc<Self>) -> Arc<[T]> {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { Arc::from_raw(Arc::into_raw(this) as *mut [T]) }
    }

    /// Constructs a `UniqueEntityEquivalentSlice` from a [`Rc<[T]>`] unsafely.
    ///
    /// # Safety
    ///
    /// `slice` must contain only unique elements.
    pub unsafe fn from_rc_slice_unchecked(slice: Rc<[T]>) -> Rc<Self> {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { Rc::from_raw(Rc::into_raw(slice) as *mut Self) }
    }

    /// Casts `self` to the inner slice.
    pub fn into_rc_inner(self: Rc<Self>) -> Rc<[T]> {
        // SAFETY: UniqueEntityEquivalentSlice is a transparent wrapper around [T].
        unsafe { Rc::from_raw(Rc::into_raw(self) as *mut [T]) }
    }

    /// Returns the first and all the rest of the elements of the slice, or `None` if it is empty.
    ///
    /// Equivalent to [`[T]::split_first`](slice::split_first).
    pub const fn split_first(&self) -> Option<(&T, &Self)> {
        let Some((first, rest)) = self.0.split_first() else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        Some((first, unsafe { Self::from_slice_unchecked(rest) }))
    }

    /// Returns the last and all the rest of the elements of the slice, or `None` if it is empty.
    ///
    /// Equivalent to [`[T]::split_last`](slice::split_last).
    pub const fn split_last(&self) -> Option<(&T, &Self)> {
        let Some((last, rest)) = self.0.split_last() else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        Some((last, unsafe { Self::from_slice_unchecked(rest) }))
    }

    /// Returns an array reference to the first `N` items in the slice.
    ///
    /// Equivalent to [`[T]::first_chunk`](slice::first_chunk).
    pub const fn first_chunk<const N: usize>(&self) -> Option<&UniqueEntityEquivalentArray<T, N>> {
        let Some(chunk) = self.0.first_chunk() else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        Some(unsafe { UniqueEntityEquivalentArray::from_array_ref_unchecked(chunk) })
    }

    /// Returns an array reference to the first `N` items in the slice and the remaining slice.
    ///
    /// Equivalent to [`[T]::split_first_chunk`](slice::split_first_chunk).
    pub const fn split_first_chunk<const N: usize>(
        &self,
    ) -> Option<(
        &UniqueEntityEquivalentArray<T, N>,
        &UniqueEntityEquivalentSlice<T>,
    )> {
        let Some((chunk, rest)) = self.0.split_first_chunk() else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            Some((
                UniqueEntityEquivalentArray::from_array_ref_unchecked(chunk),
                Self::from_slice_unchecked(rest),
            ))
        }
    }

    /// Returns an array reference to the last `N` items in the slice and the remaining slice.
    ///
    /// Equivalent to [`[T]::split_last_chunk`](slice::split_last_chunk).
    pub const fn split_last_chunk<const N: usize>(
        &self,
    ) -> Option<(
        &UniqueEntityEquivalentSlice<T>,
        &UniqueEntityEquivalentArray<T, N>,
    )> {
        let Some((rest, chunk)) = self.0.split_last_chunk() else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            Some((
                Self::from_slice_unchecked(rest),
                UniqueEntityEquivalentArray::from_array_ref_unchecked(chunk),
            ))
        }
    }

    /// Returns an array reference to the last `N` items in the slice.
    ///
    /// Equivalent to [`[T]::last_chunk`](slice::last_chunk).
    pub const fn last_chunk<const N: usize>(&self) -> Option<&UniqueEntityEquivalentArray<T, N>> {
        let Some(chunk) = self.0.last_chunk() else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        Some(unsafe { UniqueEntityEquivalentArray::from_array_ref_unchecked(chunk) })
    }

    /// Returns a reference to a subslice.
    ///
    /// Equivalent to the range functionality of [`[T]::get`].
    ///
    /// Note that only the inner [`[T]::get`] supports indexing with a [`usize`].
    ///
    /// [`[T]::get`]: `slice::get`
    pub fn get<I>(&self, index: I) -> Option<&Self>
    where
        Self: Index<I>,
        I: SliceIndex<[T], Output = [T]>,
    {
        self.0.get(index).map(|slice|
            // SAFETY: All elements in the original slice are unique.
            unsafe { Self::from_slice_unchecked(slice) })
    }

    /// Returns a mutable reference to a subslice.
    ///
    /// Equivalent to the range functionality of [`[T]::get_mut`].
    ///
    /// Note that `UniqueEntityEquivalentSlice::get_mut` cannot be called with a [`usize`].
    ///
    /// [`[T]::get_mut`]: `slice::get_mut`
    pub fn get_mut<I>(&mut self, index: I) -> Option<&mut Self>
    where
        Self: Index<I>,
        I: SliceIndex<[T], Output = [T]>,
    {
        self.0.get_mut(index).map(|slice|
            // SAFETY: All elements in the original slice are unique.
            unsafe { Self::from_slice_unchecked_mut(slice) })
    }

    /// Returns a reference to a subslice, without doing bounds checking.
    ///
    /// Equivalent to the range functionality of [`[T]::get_unchecked`].
    ///
    /// Note that only the inner [`[T]::get_unchecked`] supports indexing with a [`usize`].
    ///
    /// # Safety
    ///
    /// `index` must be safe to use with [`[T]::get_unchecked`]
    ///
    /// [`[T]::get_unchecked`]: `slice::get_unchecked`
    pub unsafe fn get_unchecked<I>(&self, index: I) -> &Self
    where
        Self: Index<I>,
        I: SliceIndex<[T], Output = [T]>,
    {
        // SAFETY: All elements in the original slice are unique.
        unsafe { Self::from_slice_unchecked(self.0.get_unchecked(index)) }
    }

    /// Returns a mutable reference to a subslice, without doing bounds checking.
    ///
    /// Equivalent to the range functionality of [`[T]::get_unchecked_mut`].
    ///
    /// Note that `UniqueEntityEquivalentSlice::get_unchecked_mut` cannot be called with an index.
    ///
    /// # Safety
    ///
    /// `index` must be safe to use with [`[T]::get_unchecked_mut`]
    ///
    /// [`[T]::get_unchecked_mut`]: `slice::get_unchecked_mut`
    pub unsafe fn get_unchecked_mut<I>(&mut self, index: I) -> &mut Self
    where
        Self: Index<I>,
        I: SliceIndex<[T], Output = [T]>,
    {
        // SAFETY: All elements in the original slice are unique.
        unsafe { Self::from_slice_unchecked_mut(self.0.get_unchecked_mut(index)) }
    }

    /// Returns an unsafe mutable pointer to the slice's buffer.
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self.0.as_mut_ptr()
    }

    /// Returns the two unsafe mutable pointers spanning the slice.
    pub const fn as_mut_ptr_range(&mut self) -> Range<*mut T> {
        self.0.as_mut_ptr_range()
    }

    /// Swaps two elements in the slice.
    pub fn swap(&mut self, a: usize, b: usize) {
        self.0.swap(a, b);
    }

    /// Reverses the order of elements in the slice, in place.
    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    /// Returns an iterator over the slice.
    pub fn iter(&self) -> Iter<'_, T> {
        // SAFETY: All elements in the original slice are unique.
        unsafe { UniqueEntityIter::from_iter_unchecked(self.0.iter()) }
    }

    /// Returns an iterator over all contiguous windows of length
    /// `size`.
    ///
    /// Equivalent to [`[T]::windows`].
    ///
    /// [`[T]::windows`]: `slice::windows`
    pub fn windows(&self, size: usize) -> Windows<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe { UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.windows(size)) }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the
    /// beginning of the slice.
    ///
    /// Equivalent to [`[T]::chunks`].
    ///
    /// [`[T]::chunks`]: `slice::chunks`
    pub fn chunks(&self, chunk_size: usize) -> Chunks<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.chunks(chunk_size))
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the
    /// beginning of the slice.
    ///
    /// Equivalent to [`[T]::chunks_mut`].
    ///
    /// [`[T]::chunks_mut`]: `slice::chunks_mut`
    pub fn chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.chunks_mut(chunk_size),
            )
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the
    /// beginning of the slice.
    ///
    /// Equivalent to [`[T]::chunks_exact`].
    ///
    /// [`[T]::chunks_exact`]: `slice::chunks_exact`
    pub fn chunks_exact(&self, chunk_size: usize) -> ChunksExact<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(
                self.0.chunks_exact(chunk_size),
            )
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the
    /// beginning of the slice.
    ///
    /// Equivalent to [`[T]::chunks_exact_mut`].
    ///
    /// [`[T]::chunks_exact_mut`]: `slice::chunks_exact_mut`
    pub fn chunks_exact_mut(&mut self, chunk_size: usize) -> ChunksExactMut<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.chunks_exact_mut(chunk_size),
            )
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the end
    /// of the slice.
    ///
    /// Equivalent to [`[T]::rchunks`].
    ///
    /// [`[T]::rchunks`]: `slice::rchunks`
    pub fn rchunks(&self, chunk_size: usize) -> RChunks<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.rchunks(chunk_size))
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the end
    /// of the slice.
    ///
    /// Equivalent to [`[T]::rchunks_mut`].
    ///
    /// [`[T]::rchunks_mut`]: `slice::rchunks_mut`
    pub fn rchunks_mut(&mut self, chunk_size: usize) -> RChunksMut<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.rchunks_mut(chunk_size),
            )
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the
    /// end of the slice.
    ///
    /// Equivalent to [`[T]::rchunks_exact`].
    ///
    /// [`[T]::rchunks_exact`]: `slice::rchunks_exact`
    pub fn rchunks_exact(&self, chunk_size: usize) -> RChunksExact<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(
                self.0.rchunks_exact(chunk_size),
            )
        }
    }

    /// Returns an iterator over `chunk_size` elements of the slice at a time, starting at the end
    /// of the slice.
    ///
    /// Equivalent to [`[T]::rchunks_exact_mut`].
    ///
    /// [`[T]::rchunks_exact_mut`]: `slice::rchunks_exact_mut`
    pub fn rchunks_exact_mut(&mut self, chunk_size: usize) -> RChunksExactMut<'_, T> {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.rchunks_exact_mut(chunk_size),
            )
        }
    }

    /// Returns an iterator over the slice producing non-overlapping runs
    /// of elements using the predicate to separate them.
    ///
    /// Equivalent to [`[T]::chunk_by`].
    ///
    /// [`[T]::chunk_by`]: `slice::chunk_by`
    pub fn chunk_by<F>(&self, pred: F) -> ChunkBy<'_, F, T>
    where
        F: FnMut(&T, &T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe { UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.chunk_by(pred)) }
    }

    /// Returns an iterator over the slice producing non-overlapping mutable
    /// runs of elements using the predicate to separate them.
    ///
    /// Equivalent to [`[T]::chunk_by_mut`].
    ///
    /// [`[T]::chunk_by_mut`]: `slice::chunk_by_mut`
    pub fn chunk_by_mut<F>(&mut self, pred: F) -> ChunkByMut<'_, F, T>
    where
        F: FnMut(&T, &T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.chunk_by_mut(pred),
            )
        }
    }

    /// Divides one slice into two at an index.
    ///
    /// Equivalent to [`[T]::split_at`](slice::split_at).
    pub const fn split_at(&self, mid: usize) -> (&Self, &Self) {
        let (left, right) = self.0.split_at(mid);
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            (
                Self::from_slice_unchecked(left),
                Self::from_slice_unchecked(right),
            )
        }
    }

    /// Divides one mutable slice into two at an index.
    ///
    /// Equivalent to [`[T]::split_at_mut`](slice::split_at_mut).
    pub const fn split_at_mut(&mut self, mid: usize) -> (&mut Self, &mut Self) {
        let (left, right) = self.0.split_at_mut(mid);
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            (
                Self::from_slice_unchecked_mut(left),
                Self::from_slice_unchecked_mut(right),
            )
        }
    }

    /// Divides one slice into two at an index, without doing bounds checking.
    ///
    /// Equivalent to [`[T]::split_at_unchecked`](slice::split_at_unchecked).
    ///
    /// # Safety
    ///
    /// `mid` must be safe to use in [`[T]::split_at_unchecked`].
    ///
    /// [`[T]::split_at_unchecked`]: `slice::split_at_unchecked`
    pub const unsafe fn split_at_unchecked(&self, mid: usize) -> (&Self, &Self) {
        // SAFETY: The safety contract is upheld by the caller.
        let (left, right) = unsafe { self.0.split_at_unchecked(mid) };
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            (
                Self::from_slice_unchecked(left),
                Self::from_slice_unchecked(right),
            )
        }
    }

    /// Divides one mutable slice into two at an index, without doing bounds checking.
    ///
    /// Equivalent to [`[T]::split_at_mut_unchecked`](slice::split_at_mut_unchecked).
    ///
    /// # Safety
    ///
    /// `mid` must be safe to use in [`[T]::split_at_mut_unchecked`].
    ///
    /// [`[T]::split_at_mut_unchecked`]: `slice::split_at_mut_unchecked`
    pub const unsafe fn split_at_mut_unchecked(&mut self, mid: usize) -> (&mut Self, &mut Self) {
        // SAFETY: The safety contract is upheld by the caller.
        let (left, right) = unsafe { self.0.split_at_mut_unchecked(mid) };
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            (
                Self::from_slice_unchecked_mut(left),
                Self::from_slice_unchecked_mut(right),
            )
        }
    }

    /// Divides one slice into two at an index, returning `None` if the slice is
    /// too short.
    ///
    /// Equivalent to [`[T]::split_at_checked`](slice::split_at_checked).
    pub const fn split_at_checked(&self, mid: usize) -> Option<(&Self, &Self)> {
        let Some((left, right)) = self.0.split_at_checked(mid) else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            Some((
                Self::from_slice_unchecked(left),
                Self::from_slice_unchecked(right),
            ))
        }
    }

    /// Divides one mutable slice into two at an index, returning `None` if the
    /// slice is too short.
    ///
    /// Equivalent to [`[T]::split_at_mut_checked`](slice::split_at_mut_checked).
    pub const fn split_at_mut_checked(&mut self, mid: usize) -> Option<(&mut Self, &mut Self)> {
        let Some((left, right)) = self.0.split_at_mut_checked(mid) else {
            return None;
        };
        // SAFETY: All elements in the original slice are unique.
        unsafe {
            Some((
                Self::from_slice_unchecked_mut(left),
                Self::from_slice_unchecked_mut(right),
            ))
        }
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`.
    ///
    /// Equivalent to [`[T]::split`].
    ///
    /// [`[T]::split`]: `slice::split`
    pub fn split<F>(&self, pred: F) -> Split<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe { UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.split(pred)) }
    }

    /// Returns an iterator over mutable subslices separated by elements that
    /// match `pred`.
    ///
    /// Equivalent to [`[T]::split_mut`].
    ///
    /// [`[T]::split_mut`]: `slice::split_mut`
    pub fn split_mut<F>(&mut self, pred: F) -> SplitMut<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.split_mut(pred),
            )
        }
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`.
    ///
    /// Equivalent to [`[T]::split_inclusive`].
    ///
    /// [`[T]::split_inclusive`]: `slice::split_inclusive`
    pub fn split_inclusive<F>(&self, pred: F) -> SplitInclusive<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.split_inclusive(pred))
        }
    }

    /// Returns an iterator over mutable subslices separated by elements that
    /// match `pred`.
    ///
    /// Equivalent to [`[T]::split_inclusive_mut`].
    ///
    /// [`[T]::split_inclusive_mut`]: `slice::split_inclusive_mut`
    pub fn split_inclusive_mut<F>(&mut self, pred: F) -> SplitInclusiveMut<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.split_inclusive_mut(pred),
            )
        }
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`, starting at the end of the slice and working backwards.
    ///
    /// Equivalent to [`[T]::rsplit`].
    ///
    /// [`[T]::rsplit`]: `slice::rsplit`
    pub fn rsplit<F>(&self, pred: F) -> RSplit<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe { UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.rsplit(pred)) }
    }

    /// Returns an iterator over mutable subslices separated by elements that
    /// match `pred`, starting at the end of the slice and working
    /// backwards.
    ///
    /// Equivalent to [`[T]::rsplit_mut`].
    ///
    /// [`[T]::rsplit_mut`]: `slice::rsplit_mut`
    pub fn rsplit_mut<F>(&mut self, pred: F) -> RSplitMut<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.rsplit_mut(pred),
            )
        }
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred`, limited to returning at most `n` items.
    ///
    /// Equivalent to [`[T]::splitn`].
    ///
    /// [`[T]::splitn`]: `slice::splitn`
    pub fn splitn<F>(&self, n: usize, pred: F) -> SplitN<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.splitn(n, pred))
        }
    }

    /// Returns an iterator over mutable subslices separated by elements that match
    /// `pred`, limited to returning at most `n` items.
    ///
    /// Equivalent to [`[T]::splitn_mut`].
    ///
    /// [`[T]::splitn_mut`]: `slice::splitn_mut`
    pub fn splitn_mut<F>(&mut self, n: usize, pred: F) -> SplitNMut<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.splitn_mut(n, pred),
            )
        }
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred` limited to returning at most `n` items.
    ///
    /// Equivalent to [`[T]::rsplitn`].
    ///
    /// [`[T]::rsplitn`]: `slice::rsplitn`
    pub fn rsplitn<F>(&self, n: usize, pred: F) -> RSplitN<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIter::from_slice_iter_unchecked(self.0.rsplitn(n, pred))
        }
    }

    /// Returns an iterator over subslices separated by elements that match
    /// `pred` limited to returning at most `n` items.
    ///
    /// Equivalent to [`[T]::rsplitn_mut`].
    ///
    /// [`[T]::rsplitn_mut`]: `slice::rsplitn_mut`
    pub fn rsplitn_mut<F>(&mut self, n: usize, pred: F) -> RSplitNMut<'_, F, T>
    where
        F: FnMut(&T) -> bool,
    {
        // SAFETY: Any subslice of a unique slice is also unique.
        unsafe {
            UniqueEntityEquivalentSliceIterMut::from_mut_slice_iter_unchecked(
                self.0.rsplitn_mut(n, pred),
            )
        }
    }

    /// Sorts the slice **without** preserving the initial order of equal elements.
    ///
    /// Equivalent to [`[T]::sort_unstable`](slice::sort_unstable).
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.0.sort_unstable();
    }

    /// Sorts the slice with a comparison function, **without** preserving the initial order of
    /// equal elements.
    ///
    /// Equivalent to [`[T]::sort_unstable_by`](slice::sort_unstable_by).
    pub fn sort_unstable_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.0.sort_unstable_by(compare);
    }

    /// Sorts the slice with a key extraction function, **without** preserving the initial order of
    /// equal elements.
    ///
    /// Equivalent to [`[T]::sort_unstable_by_key`](slice::sort_unstable_by_key).
    pub fn sort_unstable_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.0.sort_unstable_by_key(f);
    }

    /// Rotates the slice in-place such that the first `mid` elements of the
    /// slice move to the end while the last `self.len() - mid` elements move to
    /// the front.
    ///
    /// Equivalent to [`[T]::rotate_left`](slice::rotate_left).
    pub fn rotate_left(&mut self, mid: usize) {
        self.0.rotate_left(mid);
    }

    /// Rotates the slice in-place such that the first `self.len() - k`
    /// elements of the slice move to the end while the last `k` elements move
    /// to the front.
    ///
    /// Equivalent to [`[T]::rotate_right`](slice::rotate_right).
    pub fn rotate_right(&mut self, mid: usize) {
        self.0.rotate_right(mid);
    }

    /// Sorts the slice, preserving initial order of equal elements.
    ///
    /// Equivalent to [`[T]::sort`](slice::sort()).
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.0.sort();
    }

    /// Sorts the slice with a comparison function, preserving initial order of equal elements.
    ///
    /// Equivalent to [`[T]::sort_by`](slice::sort_by).
    pub fn sort_by<F>(&mut self, compare: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.0.sort_by(compare);
    }

    /// Sorts the slice with a key extraction function, preserving initial order of equal elements.
    ///
    /// Equivalent to [`[T]::sort_by_key`](slice::sort_by_key).
    pub fn sort_by_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.0.sort_by_key(f);
    }

    /// Sorts the slice with a key extraction function, preserving initial order of equal elements.
    ///
    /// Equivalent to [`[T]::sort_by_cached_key`](slice::sort_by_cached_key).
    pub fn sort_by_cached_key<K, F>(&mut self, f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        self.0.sort_by_cached_key(f);
    }

    /// Copies self into a new `UniqueEntityEquivalentVec`.
    pub fn to_vec(&self) -> UniqueEntityEquivalentVec<T>
    where
        T: Clone,
    {
        // SAFETY: All elements in the original slice are unique.
        unsafe { UniqueEntityEquivalentVec::from_vec_unchecked(self.0.to_vec()) }
    }

    /// Converts `self` into a vector without clones or allocation.
    ///
    /// Equivalent to [`[T]::into_vec`](slice::into_vec).
    pub fn into_vec(self: Box<Self>) -> UniqueEntityEquivalentVec<T> {
        // SAFETY:
        // This matches the implementation of `slice::into_vec`.
        // All elements in the original slice are unique.
        unsafe {
            let len = self.len();
            let vec = Vec::from_raw_parts(Box::into_raw(self).cast::<T>(), len, len);
            UniqueEntityEquivalentVec::from_vec_unchecked(vec)
        }
    }

    /// Converts a reference to T into a slice of length 1 (without copying).
    pub const fn from_ref(s: &T) -> &Self {
        // SAFETY: A slice with a length of 1 is always unique.
        unsafe { UniqueEntityEquivalentSlice::from_slice_unchecked(slice::from_ref(s)) }
    }

    /// Converts a reference to T into a slice of length 1 (without copying).
    pub const fn from_mut(s: &mut T) -> &mut Self {
        // SAFETY: A slice with a length of 1 is always unique.
        unsafe { UniqueEntityEquivalentSlice::from_slice_unchecked_mut(slice::from_mut(s)) }
    }

    /// Forms a slice from a pointer and a length.
    ///
    /// Equivalent to [`slice::from_raw_parts`].
    ///
    /// # Safety
    ///
    /// [`slice::from_raw_parts`] must be safe to call with `data` and `len`.
    /// Additionally, all elements in the resulting slice must be unique.
    pub const unsafe fn from_raw_parts<'a>(
        data: *const T,
        len: usize,
    ) -> &'a Self {
        // SAFETY: The safety contract is upheld by the caller.
        unsafe { UniqueEntityEquivalentSlice::from_slice_unchecked(slice::from_raw_parts(data, len)) }
    }

    /// Performs the same functionality as [`from_raw_parts`], except that a mutable slice is returned.
    ///
    /// Equivalent to [`slice::from_raw_parts_mut`].
    ///
    /// # Safety
    ///
    /// [`slice::from_raw_parts_mut`] must be safe to call with `data` and `len`.
    /// Additionally, all elements in the resulting slice must be unique.
    pub const unsafe fn from_raw_parts_mut<'a>(
        data: *mut T,
        len: usize,
    ) -> &'a mut Self {
        // SAFETY: The safety contract is upheld by the caller.
        unsafe {
            UniqueEntityEquivalentSlice::from_slice_unchecked_mut(slice::from_raw_parts_mut(data, len))
        }
    }
}

/// Casts a slice of entity slices to a slice of [`UniqueEntityEquivalentSlice`]s.
///
/// # Safety
///
/// All elements in each of the cast slices must be unique.
pub unsafe fn cast_slice_of_unique_entity_slice<'a, 'b, T: EntityEquivalent + 'a>(
    slice: &'b [&'a [T]],
) -> &'b [&'a UniqueEntityEquivalentSlice<T>] {
    // SAFETY: All elements in the original iterator are unique slices.
    unsafe { &*(ptr::from_ref(slice) as *const [&UniqueEntityEquivalentSlice<T>]) }
}

/// Casts a mutable slice of entity slices to a slice of [`UniqueEntityEquivalentSlice`]s.
///
/// # Safety
///
/// All elements in each of the cast slices must be unique.
pub unsafe fn cast_slice_of_unique_entity_slice_mut<'a, 'b, T: EntityEquivalent + 'a>(
    slice: &'b mut [&'a [T]],
) -> &'b mut [&'a UniqueEntityEquivalentSlice<T>] {
    // SAFETY: All elements in the original iterator are unique slices.
    unsafe { &mut *(ptr::from_mut(slice) as *mut [&UniqueEntityEquivalentSlice<T>]) }
}

/// Casts a mutable slice of mutable entity slices to a slice of mutable [`UniqueEntityEquivalentSlice`]s.
///
/// # Safety
///
/// All elements in each of the cast slices must be unique.
pub unsafe fn cast_slice_of_mut_unique_entity_slice_mut<'a, 'b, T: EntityEquivalent + 'a>(
    slice: &'b mut [&'a mut [T]],
) -> &'b mut [&'a mut UniqueEntityEquivalentSlice<T>] {
    // SAFETY: All elements in the original iterator are unique slices.
    unsafe { &mut *(ptr::from_mut(slice) as *mut [&mut UniqueEntityEquivalentSlice<T>]) }
}

impl<'a, T: EntityEquivalent> IntoIterator for &'a UniqueEntityEquivalentSlice<T> {
    type Item = &'a T;

    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: EntityEquivalent> IntoIterator for &'a Box<UniqueEntityEquivalentSlice<T>> {
    type Item = &'a T;

    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T: EntityEquivalent> IntoIterator for Box<UniqueEntityEquivalentSlice<T>> {
    type Item = T;

    type IntoIter = unique_vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.into_vec().into_iter()
    }
}

impl<T: EntityEquivalent> Deref for UniqueEntityEquivalentSlice<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: EntityEquivalent> AsRef<[T]> for UniqueEntityEquivalentSlice<T> {
    fn as_ref(&self) -> &[T] {
        self
    }
}

impl<T: EntityEquivalent> AsRef<Self> for UniqueEntityEquivalentSlice<T> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: EntityEquivalent> AsMut<Self> for UniqueEntityEquivalentSlice<T> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: EntityEquivalent> Borrow<[T]> for UniqueEntityEquivalentSlice<T> {
    fn borrow(&self) -> &[T] {
        self
    }
}
// TODO!



/// An iterator that yields `&UniqueEntityEquivalentSlice`. Note that an entity may appear
/// in multiple slices, depending on the wrapped iterator.
#[repr(transparent)]
#[derive(Debug)]
pub struct UniqueEntityEquivalentSliceIter<
    'a,
    T: EntityEquivalent + 'a,
    I: Iterator<Item = &'a [T]>,
> {
    iter: I,
}

/// An iterator that yields `&mut UniqueEntityEquivalentSlice`. Note that an entity may appear
/// in multiple slices, depending on the wrapped iterator.
#[repr(transparent)]
#[derive(Debug)]
pub struct UniqueEntityEquivalentSliceIterMut<
    'a,
    T: EntityEquivalent + 'a,
    I: Iterator<Item = &'a mut [T]>,
> {
    iter: I,
}
