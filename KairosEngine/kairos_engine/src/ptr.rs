use std::{fmt::{self, Debug, Pointer, Formatter}, marker::PhantomData, ptr::{self, NonNull}};




/// Used as a type argument to [`Ptr`], [`PtrMut`], [`OwningPtr`], and [`MovingPtr`] to specify that the pointer is guaranteed
/// to be [aligned].
///
/// [aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
#[derive(Debug, Clone, Copy)]
pub struct Aligned;

/// Used as a type argument to [`Ptr`], [`PtrMut`], [`OwningPtr`], and [`MovingPtr`] to specify that the pointer may not [aligned].
///
/// [aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
#[derive(Debug, Clone, Copy)]
pub struct Unaligned;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Aligned {}
    impl Sealed for super::Unaligned {}
}

/// Trait that is only implemented for [`Aligned`] and [`Unaligned`] to work around the lack of ability
/// to have const generics of an enum.
pub trait IsAligned: sealed::Sealed  {
    /// Reads the value pointed to by `ptr`.
    ///
    /// # Safety
    ///  - `ptr` must be valid for reads.
    ///  - `ptr` must point to a valid instance of type `T`
    ///  - If this type is [`Aligned`], then `ptr` must be [properly aligned] for type `T`.
    ///
    /// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
    #[doc(hidden)]
    unsafe fn read_ptr<T>(ptr: *const T) -> T;

    /// Copies `count * size_of::<T>()` bytes from `src` to `dst`. The source
    /// and destination must *not* overlap.
    ///
    /// # Safety
    ///  - `src` must be valid for reads of `count * size_of::<T>()` bytes.
    ///  - `dst` must be valid for writes of `count * size_of::<T>()` bytes.
    ///  - The region of memory beginning at `src` with a size of `count *
    ///    size_of::<T>()` bytes must *not* overlap with the region of memory
    ///    beginning at `dst` with the same size.
    ///  - If this type is [`Aligned`], then both `src` and `dst` must properly
    ///    be aligned for values of type `T`.
    #[doc(hidden)]
    unsafe fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize);

    /// Reads the value pointed to by `ptr`.
    ///
    /// # Safety
    ///  - `ptr` must be valid for reads and writes.
    ///  - `ptr` must point to a valid instance of type `T`
    ///  - If this type is [`Aligned`], then `ptr` must be [properly aligned] for type `T`.
    ///  - The value pointed to by `ptr` must be valid for dropping.
    ///  - While `drop_in_place` is executing, the only way to access parts of `ptr` is through
    ///    the `&mut Self` supplied to it's `Drop::drop` impl.
    ///
    /// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
    #[doc(hidden)]
    unsafe fn drop_in_place<T>(ptr: *mut T);
}

impl IsAligned for Aligned {
    #[inline]
    unsafe fn read_ptr<T>(ptr: *const T) -> T {
        // SAFETY:
        //  - The caller is required to ensure that `src` must be valid for reads.
        //  - The caller is required to ensure that `src` points to a valid instance of type `T`.
        //  - This type is `Aligned` so the caller must ensure that `src` is properly aligned for type `T`.
        unsafe { ptr.read() }
    }

    #[inline]
    unsafe fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize) {
        // SAFETY:
        //  - The caller is required to ensure that `src` must be valid for reads.
        //  - The caller is required to ensure that `dst` must be valid for writes.
        //  - The caller is required to ensure that `src` and `dst` are aligned.
        //  - The caller is required to ensure that the memory region covered by `src`
        //    and `dst`, fitting up to `count` elements do not overlap.
        unsafe {
            ptr::copy_nonoverlapping(src, dst, count);
        }
    }

    #[inline]
    unsafe fn drop_in_place<T>(ptr: *mut T) {
        // SAFETY:
        //  - The caller is required to ensure that `ptr` must be valid for reads and writes.
        //  - The caller is required to ensure that `ptr` points to a valid instance of type `T`.
        //  - This type is `Aligned` so the caller must ensure that `ptr` is properly aligned for type `T`.
        //  - The caller is required to ensure that `ptr` points must be valid for dropping.
        //  - The caller is required to ensure that the value `ptr` points must not be used after this function
        //    call.
        unsafe {
            ptr::drop_in_place(ptr);
        }
    }
}

impl IsAligned for Unaligned {
    #[inline]
    unsafe fn read_ptr<T>(ptr: *const T) -> T {
        // SAFETY:
        //  - The caller is required to ensure that `src` must be valid for reads.
        //  - The caller is required to ensure that `src` points to a valid instance of type `T`.
        unsafe { ptr.read_unaligned() }
    }

    #[inline]
    unsafe fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize) {
        // SAFETY:
        //  - The caller is required to ensure that `src` must be valid for reads.
        //  - The caller is required to ensure that `dst` must be valid for writes.
        //  - This is doing a byte-wise copy. `src` and `dst` are always guaranteed to be
        //    aligned.
        //  - The caller is required to ensure that the memory region covered by `src`
        //    and `dst`, fitting up to `count` elements do not overlap.
        unsafe {
            ptr::copy_nonoverlapping::<u8>(
                src.cast::<u8>(),
                dst.cast::<u8>(),
                count * size_of::<T>(),
            );
        }
    }

    #[inline]
    unsafe fn drop_in_place<T>(ptr: *mut T) {
        // SAFETY:
        //  - The caller is required to ensure that `ptr` must be valid for reads and writes.
        //  - The caller is required to ensure that `ptr` points to a valid instance of type `T`.
        //  - This type is not `Aligned` so the caller does not need to ensure that `ptr` is properly aligned for type `T`.
        //  - The caller is required to ensure that `ptr` points must be valid for dropping.
        //  - The caller is required to ensure that the value `ptr` points must not be used after this function
        //    call.
        unsafe {
            drop(ptr.read_unaligned());
        }
    }
}

/// Type-erased [`Box`]-like pointer to some unknown type chosen when constructing this type.
///
/// Conceptually represents ownership of whatever data is being pointed to and so is
/// responsible for calling its `Drop` impl. This pointer is _not_ responsible for freeing
/// the memory pointed to by this pointer as it may be pointing to an element in a `Vec` or
/// to a local in a function etc.
///
/// This type tries to act "borrow-like" which means that:
/// - Pointer should be considered exclusive and mutable. It cannot be cloned as this would lead
///   to aliased mutability and potentially use after free bugs.
/// - It must always point to a valid value of whatever the pointee type is.
/// - The lifetime `'a` accurately represents how long the pointer is valid for.
/// - If `A` is [`Aligned`], the pointer must always be [properly aligned] for the unknown pointee type.
///
/// It may be helpful to think of this type as similar to `&'a mut ManuallyDrop<dyn Any>` but
/// without the metadata and able to point to data that does not correspond to a Rust type.
///
/// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
/// [`Box`]: https://doc.rust-lang.org/std/boxed/struct.Box.html
#[repr(transparent)]
pub struct OwningPtr<'a, A: IsAligned = Aligned>(NonNull<u8>, PhantomData<(&'a mut u8, A)>);


macro_rules! impl_ptr {
    ($ptr:ident) => {
        impl<'a> $ptr<'a, Aligned> {
            /// Removes the alignment requirement of this pointer
            pub fn to_unaligned(self) -> $ptr<'a, Unaligned> {
                $ptr(self.0, PhantomData)
            }
        }

        impl<'a, A: IsAligned> From<$ptr<'a, A>> for NonNull<u8> {
            fn from(ptr: $ptr<'a, A>) -> Self {
                ptr.0
            }
        }

        impl<A: IsAligned> $ptr<'_, A> {
            /// Calculates the offset from a pointer.
            /// As the pointer is type-erased, there is no size information available. The provided
            /// `count` parameter is in raw bytes.
            ///
            /// *See also: [`ptr::offset`][ptr_offset]*
            ///
            /// # Safety
            /// - The offset cannot make the existing ptr null, or take it out of bounds for its allocation.
            /// - If the `A` type parameter is [`Aligned`] then the offset must not make the resulting pointer
            ///   be unaligned for the pointee type.
            /// - The value pointed by the resulting pointer must outlive the lifetime of this pointer.
            ///
            /// [ptr_offset]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset
            #[inline]
            pub unsafe fn byte_offset(self, count: isize) -> Self {
                Self(
                    // SAFETY: The caller upholds safety for `offset` and ensures the result is not null.
                    unsafe { NonNull::new_unchecked(self.as_ptr().offset(count)) },
                    PhantomData
                )
            }

            /// Calculates the offset from a pointer (convenience for `.offset(count as isize)`).
            /// As the pointer is type-erased, there is no size information available. The provided
            /// `count` parameter is in raw bytes.
            ///
            /// *See also: [`ptr::add`][ptr_add]*
            ///
            /// # Safety
            /// - The offset cannot make the existing ptr null, or take it out of bounds for its allocation.
            /// - If the `A` type parameter is [`Aligned`] then the offset must not make the resulting pointer
            ///   be unaligned for the pointee type.
            /// - The value pointed by the resulting pointer must outlive the lifetime of this pointer.
            ///
            /// [ptr_add]: https://doc.rust-lang.org/std/primitive.pointer.html#method.add
            pub unsafe fn byte_add(self, count: usize) -> Self {
                Self(
                    // SAFETY: The caller upholds safety for `add` and ensures the result is not null.
                    unsafe { NonNull::new_unchecked(self.as_ptr().add(count)) },
                    PhantomData
                )
            }
        }

        impl<A: IsAligned> Pointer for $ptr<'_, A> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                Pointer::fmt(&self.0, f)
            }
        }

        impl Debug for $ptr<'_, Aligned> {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{}<Aligned>({:?})", stringify!($ptr), self.0)
            }
        }

        impl Debug for $ptr<'_, Unaligned> {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                write!(f, "{}<Unaligned>({:?})", stringify!($ptr), self.0)
            }
        }
    };
}

impl_ptr!(OwningPtr);

// TODO!
