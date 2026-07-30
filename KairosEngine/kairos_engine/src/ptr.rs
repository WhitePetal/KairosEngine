use std::{fmt::{self, Debug, Formatter, Pointer}, marker::PhantomData, mem::ManuallyDrop, ptr::{self, NonNull}};




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

/// A newtype around [`NonNull`] that only allows conversion to read-only borrows or pointers.
///
/// This type can be thought of as the `*const T` to [`NonNull<T>`]'s `*mut T`.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct ConstNonNull<T: ?Sized>(NonNull<T>);

impl<T: ?Sized> ConstNonNull<T> {
    /// Creates a new `ConstNonNull` if `ptr` is non-null.
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ptr::ConstNonNull;
    ///
    /// let x = 0u32;
    /// let ptr = ConstNonNull::<u32>::new(&x as *const _).expect("ptr is null!");
    ///
    /// if let Some(ptr) = ConstNonNull::<u32>::new(core::ptr::null()) {
    ///     unreachable!();
    /// }
    /// ```
    pub fn new(ptr: *const T) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    /// Creates a new `ConstNonNull`.
    ///
    /// # Safety
    ///
    /// `ptr` must be non-null.
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ptr::ConstNonNull;
    ///
    /// let x = 0u32;
    /// let ptr = unsafe { ConstNonNull::new_unchecked(&x as *const _) };
    /// ```
    ///
    /// *Incorrect* usage of this function:
    ///
    /// ```rust,no_run
    /// use bevy_ptr::ConstNonNull;
    ///
    /// // NEVER DO THAT!!! This is undefined behavior. ⚠️
    /// let ptr = unsafe { ConstNonNull::<u32>::new_unchecked(core::ptr::null()) };
    /// ```
    pub const unsafe fn new_unchecked(ptr: *const T) -> Self {
        // SAFETY: This function's safety invariants are identical to `NonNull::new_unchecked`
        // The caller must satisfy all of them.
        unsafe { Self(NonNull::new_unchecked(ptr.cast_mut())) }
    }

    /// Returns a shared reference to the value.
    ///
    /// # Safety
    ///
    /// When calling this method, you have to ensure that all of the following is true:
    ///
    /// * The pointer must be [properly aligned].
    ///
    /// * It must be "dereferenceable" in the sense defined in [the module documentation].
    ///
    /// * The pointer must point to an initialized instance of `T`.
    ///
    /// * You must enforce Rust's aliasing rules, since the returned lifetime `'a` is
    ///   arbitrarily chosen and does not necessarily reflect the actual lifetime of the data.
    ///   In particular, while this reference exists, the memory the pointer points to must
    ///   not get mutated (except inside `UnsafeCell`).
    ///
    /// This applies even if the result of this method is unused!
    /// (The part about being initialized is not yet fully decided, but until
    /// it is, the only safe approach is to ensure that they are indeed initialized.)
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ptr::ConstNonNull;
    ///
    /// let mut x = 0u32;
    /// let ptr = ConstNonNull::new(&mut x as *mut _).expect("ptr is null!");
    ///
    /// let ref_x = unsafe { ptr.as_ref() };
    /// println!("{ref_x}");
    /// ```
    ///
    /// [the module documentation]: core::ptr#safety
    /// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
    #[inline]
    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        // SAFETY: This function's safety invariants are identical to `NonNull::as_ref`
        // The caller must satisfy all of them.
        unsafe { self.0.as_ref() }
    }
}

impl<T: ?Sized> From<NonNull<T>> for ConstNonNull<T> {
    fn from(value: NonNull<T>) -> Self {
        ConstNonNull(value)
    }
}

impl<'a, T: ?Sized> From<&'a T> for ConstNonNull<T> {
    fn from(value: &'a T) -> Self {
        ConstNonNull(NonNull::from(value))
    }
}

impl<'a, T: ?Sized> From<&'a mut T> for ConstNonNull<T> {
    fn from(value: &'a mut T) -> Self {
        ConstNonNull(NonNull::from(value))
    }
}

/// Type-erased borrow of some unknown type chosen when constructing this type.
///
/// This type tries to act "borrow-like" which means that:
/// - It should be considered immutable: its target must not be changed while this pointer is alive.
/// - It must always point to a valid value of whatever the pointee type is.
/// - The lifetime `'a` accurately represents how long the pointer is valid for.
/// - If `A` is [`Aligned`], the pointer must always be [properly aligned] for the unknown pointee type.
///
/// It may be helpful to think of this type as similar to `&'a dyn Any` but without
/// the metadata and able to point to data that does not correspond to a Rust type.
///
/// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Ptr<'a, A: IsAligned = Aligned>(NonNull<u8>, PhantomData<(&'a u8, A)>);

/// Type-erased mutable borrow of some unknown type chosen when constructing this type.
///
/// This type tries to act "borrow-like" which means that:
/// - Pointer is considered exclusive and mutable. It cannot be cloned as this would lead to
///   aliased mutability.
/// - It must always point to a valid value of whatever the pointee type is.
/// - The lifetime `'a` accurately represents how long the pointer is valid for.
/// - If `A` is [`Aligned`], the pointer must always be [properly aligned] for the unknown pointee type.
///
/// It may be helpful to think of this type as similar to `&'a mut dyn Any` but without
/// the metadata and able to point to data that does not correspond to a Rust type.
///
/// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
#[repr(transparent)]
pub struct PtrMut<'a, A: IsAligned = Aligned>(NonNull<u8>, PhantomData<(&'a mut u8, A)>);

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

impl_ptr!(Ptr);
impl_ptr!(PtrMut);
impl_ptr!(OwningPtr);


impl<'a> OwningPtr<'a> {
    unsafe fn make_internal<T>(temp: &mut ManuallyDrop<T>) -> OwningPtr<'_> {

    }

    /// Consumes a value and creates an [`OwningPtr`] to it while ensuring a double drop does not happen.
    #[inline]
    pub fn make<T, F: FnOnce(OwningPtr<'_>) -> R, R>(val: T, f: F) -> R {
        let mut val = ManuallyDrop::new(val);

        // SAFETY: The value behind the pointer will not get dropped or observed later,
        // so it's safe to promote it to an owning pointer.
        f(unsafe {
            Self::make_internal(&mut val)
        })
    }
}

impl<'a, A: IsAligned> OwningPtr<'a, A> {
    /// Creates a new instance from a raw pointer.
    ///
    /// # Safety
    /// - `inner` must point to valid value of whatever the pointee type is.
    /// - If the `A` type parameter is [`Aligned`] then `inner` must be [properly aligned] for the pointee type.
    /// - `inner` must have correct provenance to allow read and writes of the pointee type.
    /// - The lifetime `'a` must be constrained such that this [`OwningPtr`] will stay valid and nothing
    ///   else can read or mutate the pointee while this [`OwningPtr`] is live.
    ///
    /// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
    #[inline]
    pub unsafe fn new(inner: NonNull<u8>) -> Self {
        Self(inner, PhantomData)
    }

    /// Consumes the [`OwningPtr`] to obtain ownership of the underlying data of type `T`.
    ///
    /// # Safety
    /// - `T` must be the erased pointee type for this [`OwningPtr`].
    /// - If the type parameter `A` is [`Unaligned`] then this pointer must be [properly aligned]
    ///   for the pointee type `T`.
    ///
    /// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
    #[inline]
    pub unsafe fn read<T>(self) -> T {
        let ptr = self.as_ptr().cast::<T>().debug_ensure_aligned();

        // SAFETY: The caller ensure the pointee is of type `T` and uphold safety for `read`.
        unsafe { ptr.read() }
    }

    /// Consumes the [`OwningPtr`] to drop the underlying data of type `T`.
    ///
    /// # Safety
    /// - `T` must be the erased pointee type for this [`OwningPtr`].
    /// - If the type parameter `A` is [`Unaligned`] then this pointer must be [properly aligned]
    ///   for the pointee type `T`.
    ///
    /// [properly aligned]: https://doc.rust-lang.org/std/ptr/index.html#alignment
    #[inline]
    pub unsafe fn drop_as<T>(self) {
        let ptr = self.as_ptr().cast::<T>().debug_ensure_aligned();
        // SAFETY: The caller ensure the pointee is of type `T` and uphold safety for `drop_in_place`.
        unsafe {
            ptr.drop_in_place();
        }
    }

    pub unsafe fn cast<T>(self) -> MovingPtr<'a, T, A> {

    }

    /// Gets the underlying pointer, erasing the associated lifetime.
    ///
    /// If possible, it is strongly encouraged to use the other more type-safe functions
    /// over this function.
    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr()
    }

    /// Gets an immutable pointer from this owned pointer.
    #[inline]
    pub fn as_ref(&self) -> Ptr<'_, A> {
        // SAFETY: The `Owning` type's guarantees about the validity of this pointer are a superset of `Ptr` s guarantees
        unsafe { Ptr::new(self.0) }
    }

    /// Gets a mutable pointer from this owned pointer.
    #[inline]
    pub fn as_mut(&mut self) -> PtrMut<'_, A> {

    }
}


trait DebugEnsureAligned {
    fn debug_ensure_aligned(self) -> Self;
}

// Disable this for miri runs as it already checks if pointer to reference
// casts are properly aligned.
#[cfg(all(debug_assertions, not(miri)))]
impl<T: Sized> DebugEnsureAligned for *mut T {
    fn debug_ensure_aligned(self) -> Self {
        assert!(
            self.is_aligned(),
            "pointer is not aligned. Address {:p} does not have alignment {} for type {}",
            self,
            align_of::<T>(),
            std::any::type_name::<T>()
        )
    }
}

#[cfg(any(not(debug_assertions), miri))]
impl<T: Sized> DebugEnsureAligned for *mut T {
    #[inline(always)]
    fn debug_ensure_aligned(self) -> Self {
        self
    }
}

// TODO!
