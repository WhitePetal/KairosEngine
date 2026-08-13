use crate::ecs::message::{Message, Messages};

#[test]
fn iter_current_update_messages_iterates_over_current_messages() {
    // #[derive(Message, Clone)]
    #[derive(Clone)]
    struct TestMessage;

    // TODO!: use derive
    impl Message for TestMessage {}

    let mut test_messages = Messages::<TestMessage>::default();

    // Starting empty
    assert_eq!(test_messages.len(), 0);
    assert_eq!(test_messages.iter_current_update_messages().count(), 0);
    test_messages.update();

    // Writing one message
    test_messages.write(TestMessage);

    assert_eq!(test_messages.len(), 1);
    assert_eq!(test_messages.iter_current_update_messages().count(), 1);
    test_messages.update();

    // Writing two messages on the next frame
    test_messages.write(TestMessage);
    test_messages.write(TestMessage);

    assert_eq!(test_messages.len(), 3); // Messages are double-buffered, so we see 1 + 2 = 3
    assert_eq!(test_messages.iter_current_update_messages().count(), 2);
    test_messages.update();

    // Writing zero messages
    assert_eq!(test_messages.len(), 2); // Messages are double-buffered, so we see 2 + 0 = 2
    assert_eq!(test_messages.iter_current_update_messages().count(), 0);
}

#[test]
fn write_batch_iter_size_hint() {
    // #[derive(Message, Clone, Copy)]
    #[derive(Clone, Copy)]
    struct TestMessage;

    // TODO!: use derive
    impl Message for TestMessage {}

    let mut test_messages = Messages::<TestMessage>::default();
    let write_batch_ids = test_messages.write_batch([TestMessage; 4]);
    let expected_len = 4;
    assert_eq!(write_batch_ids.len(), expected_len);
    assert_eq!(
        write_batch_ids.size_hint(),
        (expected_len, Some(expected_len))
    );
}
