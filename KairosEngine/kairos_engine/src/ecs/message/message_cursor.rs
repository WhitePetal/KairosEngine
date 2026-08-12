use std::marker::PhantomData;

use crate::ecs::message::Message;


/// Stores the state for a [`MessageReader`] or [`MessageMutator`].
///
/// Access to the [`Messages<M>`] resource is required to read any incoming messages.
///
/// In almost all cases, you should just use a [`MessageReader`] to read messages,
/// or a [`MessageMutator`] to modify messages or to read and write messages in the same system,
/// which will automatically manage the state for you.
///
/// However, this type can be useful if you need to manually track messages.
///
/// # Example
///
/// ```
/// use bevy_ecs::prelude::*;
/// use bevy_ecs::message::{Message, MessageCursor};
///
/// #[derive(Message, Clone, Debug)]
/// struct MyMessage;
///
/// /// A system that both sends and receives messages using a [`Local`] [`MessageCursor`].
/// fn send_and_receive_messages(
///     // The `Local` `SystemParam` stores state inside the system itself, rather than in the world.
///     // `MessageCursor<T>` is the internal state of `MessageMutator<T>`, which tracks which messages have been seen.
///     mut local_message_reader: Local<MessageCursor<MyMessage>>,
///     // We can access the `Messages` resource mutably, allowing us to both read and write its contents.
///     mut messages: ResMut<Messages<MyMessage>>,
/// ) {
///     // We must collect the messages to resend, because we can't mutate messages while we're iterating over the messages.
///     let mut messages_to_resend = Vec::new();
///
///     for message in local_message_reader.read(&mut messages) {
///          messages_to_resend.push(message.clone());
///     }
///
///     for message in messages_to_resend {
///         messages.write(MyMessage);
///     }
/// }
///
/// # bevy_ecs::system::assert_is_system(send_and_receive_messages);
/// ```
///
/// [`MessageReader`]: super::MessageReader
/// [`MessageMutator`]: super::MessageMutator
#[derive(Debug)]
pub struct MessageCursor<M: Message> {
    pub(super) last_message_count: usize,
    pub(super) _marker: PhantomData<M>,
}

// TODO!
