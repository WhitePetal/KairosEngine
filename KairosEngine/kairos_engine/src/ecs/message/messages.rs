use std::{marker::PhantomData, ops::{Deref, DerefMut}};

use crate::{debug::MaybeLocation, ecs::{component::Component, message::{Message, MessageId, MessageInstance}, resource::Resource}};


/// A message collection that represents the messages that occurred within the last two
/// [`Messages::update`] calls.
/// Messages can be written to using a [`MessageWriter`]
/// and are typically cheaply read using a [`MessageReader`].
///
/// Each message can be consumed by multiple systems, in parallel,
/// with consumption tracked by the [`MessageReader`] on a per-system basis.
///
/// If no [ordering](https://github.com/bevyengine/bevy/blob/main/examples/ecs/ecs_guide.rs)
/// is applied between writing and reading systems, there is a risk of a race condition.
/// This means that whether the messages arrive before or after the next [`Messages::update`] is unpredictable.
///
/// This collection is meant to be paired with a system that calls
/// [`Messages::update`] exactly once per update/frame.
///
/// [`message_update_system`] is a system that does this, typically initialized automatically using
/// [`add_message`](https://docs.rs/bevy/*/bevy/app/struct.App.html#method.add_message).
/// [`MessageReader`]s are expected to read messages from this collection at least once per loop/frame.
/// Messages will persist across a single frame boundary and so ordering of message producers and
/// consumers is not critical (although poorly-planned ordering may cause accumulating lag).
/// If messages are not handled by the end of the frame after they are updated, they will be
/// dropped silently.
///
/// # Example
///
/// ```
/// use bevy_ecs::message::{Message, Messages};
///
/// #[derive(Message)]
/// struct MyMessage {
///     value: usize
/// }
///
/// // setup
/// let mut messages = Messages::<MyMessage>::default();
/// let mut cursor = messages.get_cursor();
///
/// // run this once per update/frame
/// messages.update();
///
/// // somewhere else: write a message
/// messages.write(MyMessage { value: 1 });
///
/// // somewhere else: read the messages
/// for message in cursor.read(&messages) {
///     assert_eq!(message.value, 1)
/// }
///
/// // messages are only processed once per reader
/// assert_eq!(cursor.read(&messages).count(), 0);
/// ```
///
/// # Details
///
/// [`Messages`] is implemented using a variation of a double buffer strategy.
/// Each call to [`update`](Messages::update) swaps buffers and clears out the oldest one.
/// - [`MessageReader`]s will read messages from both buffers.
/// - [`MessageReader`]s that read at least once per update will never drop messages.
/// - [`MessageReader`]s that read once within two updates might still receive some messages
/// - [`MessageReader`]s that read after two updates are guaranteed to drop all messages that occurred
///   before those updates.
///
/// The buffers in [`Messages`] will grow indefinitely if [`update`](Messages::update) is never called.
///
/// An alternative call pattern would be to call [`update`](Messages::update)
/// manually across frames to control when messages are cleared.
/// This complicates consumption and risks ever-expanding memory usage if not cleaned up,
/// but can be done by adding your message as a resource instead of using
/// [`add_message`](https://docs.rs/bevy/*/bevy/app/struct.App.html#method.add_message).
///
/// [Example usage.](https://github.com/bevyengine/bevy/blob/latest/examples/ecs/message.rs)
/// [Example usage standalone.](https://github.com/bevyengine/bevy/blob/latest/crates/bevy_ecs/examples/messages.rs)
///
/// [`MessageReader`]: super::MessageReader
/// [`MessageWriter`]: super::MessageWriter
/// [`message_update_system`]: super::message_update_system
// #[derive(Debug, Resource)]
// #[cfg_attr(feature = "bevy_reflect", derive(Reflect), reflect(Resource, Default))]
#[derive(Debug)]
pub struct Messages<M: Message> {
    /// Holds the oldest still active messages.
    /// Note that `a.start_message_count + a.len()` should always be equal to `messages_b.start_message_count`.
    pub(crate) messages_a: MessageSequence<M>,
    /// Holds the newer messages.
    pub(crate) messages_b: MessageSequence<M>,
    pub(crate) message_count: usize,
}

// Derived Default impl would incorrectly require M: Default
impl<M: Message> Default for Messages<M> {
    fn default() -> Self {
        Self {
            messages_a: Default::default(),
            messages_b: Default::default(),
            message_count: Default::default()
        }
    }
}

impl<M: Message> Messages<M> {
    /// Returns the index of the oldest message stored in the message buffer.
    pub fn oldest_message_count(&self) -> usize {
        self.messages_a.start_message_count
    }

    /// Writes an `message` to the current message buffer.
    /// [`MessageReader`](super::MessageReader)s can then read the message.
    /// This method returns the [ID](`MessageId`) of the written `message`.
    #[track_caller]
    pub fn write(&mut self, message: M) -> MessageId<M> {
        self.write_with_caller(message, MaybeLocation::caller())
    }

    pub(crate) fn write_with_caller(&mut self, message: M, caller: MaybeLocation) -> MessageId<M> {
        let message_id = MessageId {
            id: self.message_count,
            caller,
            _marker: PhantomData,
        };
        // #[cfg(feature = "detailed_trace")]
        // tracing::trace!("Messages::write() -> id: {}", message_id);

        let message_instance = MessageInstance {
            message_id,
            message
        };

        self.messages_b.push(message_instance);
        self.message_count += 1;

        message_id
    }

    /// Writes a list of `messages` all at once, which can later be read by [`MessageReader`](super::MessageReader)s.
    /// This is more efficient than writing each message individually.
    /// This method returns the [IDs](`MessageId`) of the written `messages`.
    #[track_caller]
    pub fn write_batch(&mut self, messages: impl IntoIterator<Item = M>) -> WriteBatchIds<M> {
        let last_count = self.message_count;

        self.extend(messages);

        WriteBatchIds {
            last_count,
            message_count: self.message_count,
            _marker: PhantomData
        }
    }
}

impl<M: Message> Extend<M> for Messages<M> {
    fn extend<I: IntoIterator<Item = M>>(&mut self, iter: I) {
        let old_count = self.message_count;
        let mut message_count = self.message_count;
        let messages = iter.into_iter().map(|message| {
            let message_id = MessageId {
                id: message_count,
                caller: MaybeLocation::caller(),
                _marker: PhantomData
            };
            message_count += 1;
            MessageInstance {
                message_id,
                message
            }
        });

        self.messages_b.extend(messages);

        if old_count != message_count {
            // #[cfg(feature = "detailed_trace")]
            // tracing::trace!(
            //     "Messages::extend() -> ids: ({}..{})",
            //     self.message_count,
            //     message_count
            // );
            self.message_count = message_count;
        }
    }
}

// TODO!: use derive
impl<M: Message> Component for Messages<M> {
    const STORAGE_TYPE: crate::ecs::component::StorageType = crate::ecs::component::StorageType::SparseSet;

    type Mutability = crate::ecs::component::Mutable;
}

impl<M: Message> Resource for Messages<M> {

}


#[derive(Debug)]
// #[cfg_attr(feature = "bevy_reflect", derive(Reflect), reflect(Default))]
pub(crate) struct MessageSequence<M: Message> {
    pub(crate) messages: Vec<MessageInstance<M>>,
    pub(crate) start_message_count: usize,
}

// Derived Default impl would incorrectly require M: Default
impl<M: Message> Default for MessageSequence<M> {
    fn default() -> Self {
        Self {
            messages: Default::default(),
            start_message_count: Default::default()
        }
    }
}

impl<M: Message> Deref for MessageSequence<M> {
    type Target = Vec<MessageInstance<M>>;

    fn deref(&self) -> &Self::Target {
        &self.messages
    }
}

impl<M: Message> DerefMut for MessageSequence<M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.messages
    }
}

/// [`Iterator`] over written [`MessageIds`](`MessageId`) from a batch.
pub struct WriteBatchIds<M> {
    last_count: usize,
    message_count: usize,
    _marker: PhantomData<M>,
}

impl<M: Message> Iterator for WriteBatchIds<M> {
    type Item = MessageId<M>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.last_count >= self.message_count {
            return None;
        }

        let result = Some(MessageId {
            id: self.last_count,
            caller: MaybeLocation::caller(),
            _marker: PhantomData
        });

        self.last_count += 1;

        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = <Self as ExactSizeIterator>::len(self);
        (len, Some(len))
    }
}

impl<M: Message> ExactSizeIterator for WriteBatchIds<M> {
    fn len(&self) -> usize {
        self.message_count.saturating_sub(self.last_count)
    }
}
