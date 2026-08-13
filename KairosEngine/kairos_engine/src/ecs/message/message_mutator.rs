use crate::ecs::{
    change_detection::ResMut,
    message::{Message, MessageCursor, Messages},
    system::{Local, ReadOnlySystemParam, SystemParam, SystemParamValidationError},
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
// #[derive(SystemParam, Debug)]
#[derive(Debug)]
pub struct MessageMutator<'w, 's, M: Message> {
    pub(super) reader: Local<'s, MessageCursor<M>>,
    messages: ResMut<'w, Messages<M>>,
}

// TODO!: use derive
const _: () = {
    type __StructFieldsAlias<'w, 's, M> = (Local<'s, MessageCursor<M>>, ResMut<'w, Messages<M>>);

    pub struct FetchState<M: Message> {
        // 实际类型是(SyncCellM<MessageCursor<M>>, ComponentId)
        state: <__StructFieldsAlias<'static, 'static, M> as SystemParam>::State,
    }

    unsafe impl<M: Message> SystemParam for MessageMutator<'_, '_, M> {
        type State = FetchState<M>;

        type Item<'w, 's> = MessageMutator<'w, 's, M>;

        fn init_state(world: &mut crate::ecs::world::World) -> Self::State {
            // (SyncCellM<MessageCursor<M>>::default(), world.components_registrator().register_component::<T>())
            FetchState {
                state: <__StructFieldsAlias<'_, '_, M> as SystemParam>::init_state(world),
            }
        }

        fn init_access(
            state: &Self::State,
            system_meta: &mut crate::ecs::system::SystemMeta,
            component_access_set: &mut crate::ecs::query::FilteredAccessSet,
            world: &mut crate::ecs::world::World,
        ) {
            <__StructFieldsAlias<'_, '_, M> as SystemParam>::init_access(
                &state.state,
                system_meta,
                component_access_set,
                world,
            );
        }

        fn apply(
            state: &mut Self::State,
            system_meta: &crate::ecs::system::SystemMeta,
            world: &mut crate::ecs::world::World,
        ) {
            <__StructFieldsAlias<'_, '_, M> as SystemParam>::apply(
                &mut state.state,
                system_meta,
                world,
            );
        }

        fn queue(
            state: &mut Self::State,
            system_meta: &crate::ecs::system::SystemMeta,
            world: crate::ecs::world::DeferredWorld,
        ) {
            <__StructFieldsAlias<'_, '_, M> as SystemParam>::queue(
                &mut state.state,
                system_meta,
                world,
            );
        }

        unsafe fn get_param<'w, 's>(
            state: &'s mut Self::State,
            system_meta: &crate::ecs::system::SystemMeta,
            world: crate::ecs::world::unsafe_world_cell::UnsafeWorldCell<'w>,
            change_tick: crate::ecs::change_detection::Tick,
        ) -> Result<Self::Item<'w, 's>, crate::ecs::system::SystemParamValidationError> {
            let (field0, field1) = &mut state.state;
            let field0 = unsafe {
                <Local<'s, MessageCursor<M>> as SystemParam>::get_param(
                    field0,
                    system_meta,
                    world,
                    change_tick,
                )
            }
            .map_err(|err| {
                SystemParamValidationError::new::<Self>(err.skipped, err.message, "::reader")
            })?;
            let field1 = unsafe {
                <ResMut<'w, Messages<M>> as SystemParam>::get_param(
                    field1,
                    system_meta,
                    world,
                    change_tick,
                )
            }
            .map_err(|err| {
                SystemParamValidationError::new::<Self>(
                    err.skipped,
                    "Message not initialized",
                    "::messages",
                )
            })?;
            Result::Ok(MessageMutator {
                reader: field0,
                messages: field1,
            })
        }
    }

    unsafe impl<'w, 's, M: Message> ReadOnlySystemParam for MessageMutator<'w, 's, M>
    where
        Local<'s, MessageCursor<M>>: ReadOnlySystemParam,
        ResMut<'w, Messages<M>>: ReadOnlySystemParam,
    {
    }
};
