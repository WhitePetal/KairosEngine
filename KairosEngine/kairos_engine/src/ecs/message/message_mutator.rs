use crate::ecs::{
    change_detection::ResMut,
    message::{Message, MessageCursor, Messages},
    system::{Local, SystemParam},
};

/// Reads and writes [`Message`]s of type `T`, keeping track of which messages have already been read.
///
/// This can be used if a system needs to both read and write messages of the same type.
///
/// Since it has exclusive access to the underlying messages, it also permits messages to be modified as they are read.
/// This is ideal for chains of systems that all want to modify the same messages.
///
/// # Usage
///
/// [`MessageMutator`]s are usually declared as a [`SystemParam`].
/// ```
/// # use bevy_ecs::prelude::*;
///
/// #[derive(Message, Debug)]
/// pub struct MyMessage(pub u32); // Custom message type.
/// fn my_system(mut mutator: MessageMutator<MyMessage>) {
///     // This message will be read immediately by this system,
///     // and will then be visible to other systems.
///     mutator.write(MyMessage(0));
///     for message in mutator.read() {
///         message.0 += 1;
///         println!("received message: {:?}", message);
///     }
///     // This message will be read on the next run of this system,
///     // but will be visible immediately to other systems.
///     mutator.write(MyMessage(0));
/// }
/// ```
///
/// # Concurrency
///
/// Multiple systems with `MessageMutator<T>` of the same message type can not run concurrently.
/// They also can not be executed in parallel with [`MessageReader`] or [`MessageWriter`].
///
/// # Clearing, Reading, and Peeking
///
/// Messages are stored in a double buffered queue that switches each frame. This switch also clears the previous
/// frame's messages. Messages should be read each frame otherwise they may be lost. For manual control over this
/// behavior, see [`Messages`].
///
/// Most of the time systems will want to use [`MessageMutator::read()`]. This function creates an iterator over
/// all messages that haven't been read yet by this system, marking the message as read in the process.
///
/// [`MessageReader`]: super::MessageReader
/// [`MessageWriter`]: super::MessageWriter
#[derive(SystemParam, Debug)]
pub struct MessageMutator<'w, 's, M: Message> {
    pub(super) reader: Local<'s, MessageCursor<M>>,
    #[system_param(validation_message = "Message not initialized")]
    messages: ResMut<'w, Messages<M>>,
}

// TODO!
