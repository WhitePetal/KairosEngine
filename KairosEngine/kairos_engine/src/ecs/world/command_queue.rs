use std::{
    fmt::Debug,
    mem::MaybeUninit,
    ptr::{NonNull, addr_of_mut},
};

use crate::{
    debug::MaybeLocation,
    ecs::{system::Command, world::World},
    ptr::{OwningPtr, Unaligned},
};

struct CommandMeta {
    /// SAFETY: The `value` must point to a value of type `T: Command`,
    /// where `T` is some specific type that was used to produce this metadata.
    ///
    /// `world` is optional to allow this one function pointer to perform double-duty as a drop.
    ///
    /// Advances `cursor` by the size of `T` in bytes.
    consume_command_and_get_size:
        unsafe fn(value: OwningPtr<Unaligned>, world: Option<NonNull<World>>, cursor: &mut usize),
}

/// Densely and efficiently stores a queue of heterogenous types implementing [`Command`].
// NOTE: [`CommandQueue`] is implemented via a `Vec<MaybeUninit<u8>>` instead of a `Vec<Box<dyn Command>>`
// as an optimization. Since commands are used frequently in systems as a way to spawn
// entities/components/resources, and it's not currently possible to parallelize these
// due to mutable [`World`] access, maximizing performance for [`CommandQueue`] is
// preferred to simplicity of implementation.
pub struct CommandQueue {
    // This buffer densely stores all queued commands.
    //
    // For each command, one `CommandMeta` is stored, followed by zero or more bytes
    // to store the command itself. To interpret these bytes, a pointer must
    // be passed to the corresponding `CommandMeta.apply_command_and_get_size` fn pointer.
    pub(crate) bytes: Vec<MaybeUninit<u8>>,
    pub(crate) cursor: usize,
    pub(crate) painc_recovery: Vec<MaybeUninit<u8>>,
    pub(crate) caller: MaybeLocation,
}

impl Default for CommandQueue {
    #[track_caller]
    fn default() -> Self {
        Self {
            bytes: Default::default(),
            cursor: Default::default(),
            painc_recovery: Default::default(),
            caller: MaybeLocation::caller(),
        }
    }
}

/// Wraps pointers to a [`CommandQueue`], used internally to avoid stacked borrow rules when
/// partially applying the world's command queue recursively
#[derive(Clone)]
pub(crate) struct RawCommandQueue {
    pub(crate) bytes: NonNull<Vec<MaybeUninit<u8>>>,
    pub(crate) cursor: NonNull<usize>,
    pub(crate) panic_recovery: NonNull<Vec<MaybeUninit<u8>>>,
}

// CommandQueue needs to implement Debug manually, rather than deriving it, because the derived impl just prints
// [core::mem::maybe_uninit::MaybeUninit<u8>, core::mem::maybe_uninit::MaybeUninit<u8>, ..] for every byte in the vec,
// which gets extremely verbose very quickly, while also providing no useful information.
// It is not possible to soundly print the values of the contained bytes, as some of them may be padding or uninitialized (#4863)
// So instead, the manual impl just prints the length of vec.
impl Debug for CommandQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandQueue")
            .field("len_bytes", &self.bytes.len())
            .field("caller", &self.cursor)
            .finish_non_exhaustive()
    }
}

// SAFETY: All commands [`Command`] implement [`Send`]
unsafe impl Send for CommandQueue {}

// SAFETY: `&CommandQueue` never gives access to the inner commands.
unsafe impl Sync for CommandQueue {}

impl CommandQueue {
    pub fn push(&mut self, command: impl Command<Out = ()>) {
        // SAFETY: self is guaranteed to live for the lifetime of this method
        unsafe {
            self.get_raw().push(command);
        }
    }

    /// Returns a [`RawCommandQueue`] instance sharing the underlying command queue.
    pub(crate) fn get_raw(&mut self) -> RawCommandQueue {
        // SAFETY: self is always valid memory
        unsafe {
            RawCommandQueue {
                bytes: NonNull::new_unchecked(addr_of_mut!(self.bytes)),
                cursor: NonNull::new_unchecked(addr_of_mut!(self.cursor)),
                panic_recovery: NonNull::new_unchecked(addr_of_mut!(self.painc_recovery)),
            }
        }
    }
}

impl RawCommandQueue {
    /// Returns a new `RawCommandQueue` instance, this must be manually dropped.
    pub(crate) fn new() -> Self {
        // SAFETY: Pointers returned by `Box::into_raw` are guaranteed to be non null
        unsafe {
            Self {
                bytes: NonNull::new_unchecked(Box::into_raw(Box::default())),
                cursor: NonNull::new_unchecked(Box::into_raw(Box::new(0usize))),
                panic_recovery: NonNull::new_unchecked(Box::into_raw(Box::default())),
            }
        }
    }

    /// Push a [`Command`] onto the queue.
    ///
    /// # Safety
    ///
    /// * Caller ensures that `self` has not outlived the underlying queue
    #[inline]
    pub unsafe fn push<C: Command<Out = ()>>(&mut self, command: C) {
        // Stores a command alongside its metadata.
        // `repr(C)` prevents the compiler from reordering the fields,
        // while `repr(packed)` prevents the compiler from inserting padding bytes.
        #[repr(C, packed)]
        struct Packed<C: Command<Out = ()>> {
            meta: CommandMeta,
            command: C,
        }

        let meta = CommandMeta {
            consume_command_and_get_size: |command, world, cursor| {
                *cursor += size_of::<C>();

                // SAFETY: According to the invariants of `CommandMeta.consume_command_and_get_size`,
                // `command` must point to a value of type `C`.
                let command: C = unsafe { command.read_unaligned() };
                match world {
                    // Apply command to the provided world...
                    Some(mut world) => {
                        // SAFETY: Caller ensures pointer is not null
                        let world = unsafe { world.as_mut() };
                        command.apply(world);
                        // The command may have queued up world commands, which we flush here to ensure they are also picked up.
                        // If the current command queue already the World Command queue, this will still behave appropriately because the global cursor
                        // is still at the current `stop`, ensuring only the newly queued Commands will be applied.
                        world.flush();
                    }
                    // ...or discard it.
                    None => drop(command),
                }
            },
        };

        // SAFETY: There are no outstanding references to self.bytes
        let bytes = unsafe { self.bytes.as_mut() };

        let old_len = bytes.len();

        // Reserve enough bytes for both the metadata and the command itself.
        bytes.reserve(size_of::<Packed<C>>());

        // Pointer to the bytes at the end of the buffer.
        // SAFETY: We know it is within bounds of the allocation, due to the call to `.reserve()`.
        let ptr = unsafe { bytes.as_mut_ptr().add(old_len) };

        // Write the metadata into the buffer, followed by the command.
        // We are using a packed struct to write them both as one operation.
        // SAFETY: `ptr` must be non-null, since it is within a non-null buffer.
        // The call to `reserve()` ensures that the buffer has enough space to fit a value of type `C`,
        // and it is valid to write any bit pattern since the underlying buffer is of type `MaybeUninit<u8>`.
        unsafe {
            ptr.cast::<Packed<C>>()
                .write_unaligned(Packed { meta, command });
        }

        // Extend the length of the buffer to include the data we just wrote.
        // SAFETY: The new length is guaranteed to fit in the vector's capacity,
        // due to the call to `.reserve()` above.
        unsafe {
            bytes.set_len(old_len + size_of::<Packed<C>>());
        }
    }
}
