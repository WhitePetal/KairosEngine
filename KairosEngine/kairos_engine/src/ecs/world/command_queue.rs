use std::{
    fmt::Debug,
    mem::MaybeUninit,
    panic::{AssertUnwindSafe},
    ptr::{NonNull, addr_of_mut},
};

use log::warn;

use crate::{
    debug::MaybeLocation,
    ecs::{
        system::{Command, SystemBuffer, SystemMeta},
        world::{DeferredWorld, World},
    },
    ptr::{OwningPtr, Unaligned},
};

#[cfg(test)]
mod tests;

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
    /// Push a [`Command`] onto the queue.
    #[inline]
    pub fn push(&mut self, command: impl Command<Out = ()>) {
        // SAFETY: self is guaranteed to live for the lifetime of this method
        unsafe {
            self.get_raw().push(command);
        }
    }

    /// Execute the queued [`Command`]s in the world after applying any commands in the world's internal queue.
    /// This clears the queue.
    #[inline]
    pub fn apply(&mut self, world: &mut World) {
        // flush the world's internal queue
        world.flush_commands();

        // SAFETY: A reference is always a valid pointer
        unsafe { self.get_raw().apply_or_drop_queued(Some(world.into())) }
    }

    /// Take all commands from `other` and append them to `self`, leaving `other` empty
    pub fn append(&mut self, other: &mut CommandQueue) {
        self.bytes.append(&mut other.bytes);
    }

    /// Returns false if there are any commands in the queue
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.cursor >= self.bytes.len()
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

    /// Returns true if the queue is empty.
    ///
    /// # Safety
    ///
    /// * Caller ensures that `bytes` and `cursor` point to valid memory
    pub unsafe fn is_empty(&self) -> bool {
        // SAFETY: Pointers are guaranteed to be valid by requirements on `.clone_unsafe`
        (unsafe { *self.cursor.as_ref() } >= unsafe { self.bytes.as_ref() }.len())
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

    /// If `world` is [`Some`], this will apply the queued [commands](`Command`).
    /// If `world` is [`None`], this will drop the queued [commands](`Command`) (without applying them).
    /// This clears the queue.
    ///
    /// # Safety
    ///
    /// * Caller ensures that `self` has not outlived the underlying queue
    #[inline]
    pub(crate) unsafe fn apply_or_drop_queued(&mut self, world: Option<NonNull<World>>) {
        // SAFETY: If this is the command queue on world, world will not be dropped as we have a mutable reference
        // If this is not the command queue on world we have exclusive ownership and self will not be mutated
        let start = unsafe { *self.cursor.as_ref() };
        let stop = unsafe { self.bytes.as_ref().len() };
        let mut local_cursor = start;
        // SAFETY: we are setting the global cursor to the current length to prevent the executing commands from applying
        // the remaining commands currently in this list. This is safe.
        unsafe { *self.cursor.as_mut() = stop };

        while local_cursor < stop {
            // SAFETY: The cursor is either at the start of the buffer, or just after the previous command.
            // Since we know that the cursor is in bounds, it must point to the start of a new command.
            let meta = unsafe {
                self.bytes
                    .as_mut()
                    .as_mut_ptr()
                    .add(local_cursor)
                    .cast::<CommandMeta>()
                    .read_unaligned()
            };

            // Advance to the bytes just after `meta`, which represent a type-erased command.
            local_cursor += size_of::<CommandMeta>();

            let cmd = unsafe {
                OwningPtr::<Unaligned>::new(NonNull::new_unchecked(
                    self.bytes.as_mut().as_mut_ptr().add(local_cursor).cast(),
                ))
            };
            let f = AssertUnwindSafe(|| {
                // SAFETY: The data underneath the cursor must correspond to the type erased in metadata,
                // since they were stored next to each other by `.push()`.
                // For ZSTs, the type doesn't matter as long as the pointer is non-null.
                // This also advances the cursor past the command. For ZSTs, the cursor will not move.
                // At this point, it will either point to the next `CommandMeta`,
                // or the cursor will be out of bounds and the loop will end.
                unsafe { (meta.consume_command_and_get_size)(cmd, world, &mut local_cursor) }
            });

            let result = std::panic::catch_unwind(f);

            if let Err(payload) = result {
                // local_cursor now points to the location _after_ the panicked command.
                // Add the remaining commands that _would have_ been applied to the
                // panic_recovery queue.
                //
                // This uses `current_stop` instead of `stop` to account for any commands
                // that were queued _during_ this panic.
                //
                // This is implemented in such a way that if apply_or_drop_queued() are nested recursively in,
                // an applied Command, the correct command order will be retained.
                let panic_recovery = unsafe { self.panic_recovery.as_mut() };
                let bytes = unsafe { self.bytes.as_mut() };
                let current_stop = bytes.len();
                panic_recovery.extend_from_slice(&bytes[local_cursor..current_stop]);
                unsafe {
                    bytes.set_len(start);
                    *self.cursor.as_mut() = start;
                }

                // This was the "top of the apply stack". If we are _not_ at the top of the apply stack,
                // when we call`resume_unwind" the caller "closer to the top" will catch the unwind and do this check,
                // until we reach the top.
                if start == 0 {
                    bytes.append(panic_recovery);
                }
                std::panic::resume_unwind(payload);
            }
        }

        // Reset the buffer: all commands past the original `start` cursor have been applied.
        // SAFETY: we are setting the length of bytes to the original length, minus the length of the original
        // list of commands being considered. All bytes remaining in the Vec are still valid, unapplied commands.
        unsafe {
            self.bytes.as_mut().set_len(start);
            *self.cursor.as_mut() = start;
        }
    }
}

impl Drop for CommandQueue {
    fn drop(&mut self) {
        if !self.bytes.is_empty() {
            if let Some(caller) = self.caller.into_option() {
                warn!(
                    "CommandQueue has un-applied commands being dropped. Did you forget to call SystemState::apply? caller:{caller:?}"
                );
            } else {
                warn!(
                    "CommandQueue has un-applied commands being dropped. Did you forget to call SystemState::apply?"
                );
            }
        }
        // SAFETY: A reference is always a valid pointer
        unsafe {
            self.get_raw().apply_or_drop_queued(None);
        }
    }
}

impl SystemBuffer for CommandQueue {
    #[inline]
    fn apply(&mut self, _system_meta: &SystemMeta, world: &mut World) {
        #[cfg(feature = "trace")]
        let _span_guard = _system_meta.commands_span.enter();
        self.apply(world);
    }

    #[inline]
    fn queue(&mut self, _system_meta: &SystemMeta, mut world: DeferredWorld) {
        world.commands().append(self)
    }
}
