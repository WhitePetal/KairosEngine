use std::{iter::Chain, slice::IterMut};

use crate::ecs::{
    batching::BatchingStrategy,
    message::{Message, MessageCursor, MessageId, MessageInstance, Messages},
};

/// An iterator that yields any unread messages from an [`MessageMutator`] or [`MessageCursor`].
///
/// [`MessageMutator`]: super::MessageMutator
pub struct MessageMutIterator<'a, M: Message> {
    iter: MessageMutIteratorWithId<'a, M>,
}

pub struct MessageMutIteratorWithId<'a, M: Message> {
    mutator: &'a mut MessageCursor<M>,
    chain: Chain<IterMut<'a, MessageInstance<M>>, IterMut<'a, MessageInstance<M>>>,
    unread: usize,
}

impl<'a, M: Message> Iterator for MessageMutIterator<'a, M> {
    type Item = &'a mut M;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(message, _)| message)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn count(self) -> usize {
        self.iter.count()
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        self.iter.last().map(|(message, _)| message)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.iter.nth(n).map(|(message, _)| message)
    }
}

impl<'a, M: Message> ExactSizeIterator for MessageMutIterator<'a, M> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<'a, M: Message> MessageMutIteratorWithId<'a, M> {
    /// Creates a new iterator that yields any `messages` that have not yet been seen by `mutator`.
    pub fn new(mutator: &'a mut MessageCursor<M>, messages: &'a mut Messages<M>) -> Self {
        let a_index = mutator
            .last_message_count
            .saturating_sub(messages.messages_a.start_message_count);
        let b_index = mutator
            .last_message_count
            .saturating_sub(messages.messages_b.start_message_count);
        let a = messages.messages_a.get_mut(a_index..).unwrap_or_default();
        let b = messages.messages_b.get_mut(b_index..).unwrap_or_default();

        let unread_count = a.len() + b.len();

        mutator.last_message_count = messages.message_count - unread_count;
        // Iterate the oldest first, then the newer messages
        let chain = a.iter_mut().chain(b.iter_mut());

        Self {
            mutator,
            chain,
            unread: unread_count,
        }
    }

    /// Iterate over only the messages.
    pub fn without_id(self) -> MessageMutIterator<'a, M> {
        MessageMutIterator { iter: self }
    }
}

impl<'a, M: Message> Iterator for MessageMutIteratorWithId<'a, M> {
    type Item = (&'a mut M, MessageId<M>);

    fn next(&mut self) -> Option<Self::Item> {
        match self
            .chain
            .next()
            .map(|instance| (&mut instance.message, instance.message_id))
        {
            Some(item) => {
                // #[cfg(feature = "detailed_trace")]
                // tracing::trace!("MessageMutator::iter() -> {}", item.1);
                self.mutator.last_message_count += 1;
                self.unread -= 1;
                Some(item)
            }
            None => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chain.size_hint()
    }

    fn count(self) -> usize {
        self.mutator.last_message_count += self.unread;
        self.unread
    }

    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        let MessageInstance {
            message_id,
            message,
        } = self.chain.last()?;
        self.mutator.last_message_count += self.unread;
        Some((message, *message_id))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(MessageInstance {
            message_id,
            message,
        }) = self.chain.nth(n)
        {
            self.mutator.last_message_count += n + 1;
            self.unread -= n + 1;
            Some((message, *message_id))
        } else {
            self.mutator.last_message_count += self.unread;
            self.unread = 0;
            None
        }
    }
}

impl<'a, M: Message> ExactSizeIterator for MessageMutIteratorWithId<'a, M> {
    fn len(&self) -> usize {
        self.unread
    }
}

/// A parallel iterator over `Message`s.
#[derive(Debug)]
pub struct MessageMutParIter<'a, M: Message> {
    mutator: &'a mut MessageCursor<M>,
    slices: [&'a mut [MessageInstance<M>]; 2],
    batching_strategy: BatchingStrategy,
    unread: usize,
}

impl<'a, M: Message> MessageMutParIter<'a, M> {
    /// Creates a new parallel iterator over `messages` that have not yet been seen by `mutator`.
    pub fn new(mutator: &'a mut MessageCursor<M>, messages: &'a mut Messages<M>) -> Self {
        let a_index = mutator
            .last_message_count
            .saturating_sub(messages.messages_a.start_message_count);
        let b_index = mutator
            .last_message_count
            .saturating_sub(messages.messages_b.start_message_count);
        let a = messages.messages_a.get_mut(a_index..).unwrap_or_default();
        let b = messages.messages_b.get_mut(b_index..).unwrap_or_default();

        let unread_count = a.len() + b.len();
        mutator.last_message_count = messages.message_count - unread_count;

        Self {
            mutator,
            slices: [a, b],
            batching_strategy: Default::default(),
            unread: unread_count,
        }
    }

    /// Changes the batching strategy used when iterating.
    ///
    /// For more information on how this affects the resultant iteration, see
    /// [`BatchingStrategy`].
    pub fn batching_strategy(mut self, strategy: BatchingStrategy) -> Self {
        self.batching_strategy = strategy;
        self
    }

    /// Runs the provided closure for each unread message in parallel.
    ///
    /// Unlike normal iteration, the message order is not guaranteed in any form.
    ///
    /// # Panics
    /// If the [`ComputeTaskPool`] is not initialized. If using this from a message reader that is being
    /// initialized and run from the ECS scheduler, this should never panic.
    ///
    /// [`ComputeTaskPool`]: bevy_tasks::ComputeTaskPool
    pub fn for_each<FN: Fn(&'a mut M) + Send + Sync + Clone>(self, func: FN) {
        self.for_each_with_id(move |e, _| func(e));
    }

    pub fn for_each_with_id<FN: Fn(&'a mut M, MessageId<M>) + Send + Sync + Clone>(
        mut self,
        func: FN,
    ) {
        let thread_count = rayon::current_num_threads();
        if thread_count <= 1 {
            return self.into_iter().for_each(|(e, i)| func(e, i));
        }

        let batch_size = self
            .batching_strategy
            .calc_batch_size(|| self.len(), thread_count);

        let [a, b] = self.slices;
        use rayon::prelude::*;
        a.par_chunks_mut(batch_size)
            .chain(b.par_chunks_mut(batch_size))
            .for_each(|batch| {
                for message_instnace in batch {
                    func(&mut message_instnace.message, message_instnace.message_id);
                }
            });

        // Messages are guaranteed to be read at this point.
        self.mutator.last_message_count += self.unread;
        self.unread = 0;
    }

    /// Returns the number of [`Message`]s to be iterated.
    pub fn len(&self) -> usize {
        self.slices.iter().map(|s| s.len()).sum()
    }

    /// Returns [`true`] if there are no messages remaining in this iterator.
    pub fn is_empty(&self) -> bool {
        self.slices.iter().all(|x| x.is_empty())
    }
}

impl<'a, M: Message> IntoIterator for MessageMutParIter<'a, M> {
    type Item = <Self::IntoIter as Iterator>::Item;

    type IntoIter = MessageMutIteratorWithId<'a, M>;

    fn into_iter(self) -> Self::IntoIter {
        let MessageMutParIter {
            mutator: reader,
            slices: [a, b],
            ..
        } = self;
        let unread = a.len() + b.len();
        let chain = a.iter_mut().chain(b);
        MessageMutIteratorWithId {
            mutator: reader,
            chain,
            unread,
        }
    }
}
