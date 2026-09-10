use kairos_collections::hash::fixed_hash_one;

use crate::{
    name::{
        Name,
        NameOrEntity,
    },
    world::World,
};
use serde_test::{Token, assert_tokens};

#[test]
fn test_display_of_debug_name() {
    let mut world = World::new();
    let e1 = world.spawn_empty().id();
    let name = Name::new("MyName");
    let e2 = world.spawn(name.clone()).id();
    let mut query = world.query::<NameOrEntity>();
    let d1 = query.get(&world, e1).unwrap();
    // NameOrEntity Display for entities without a Name should be {index}v{generation}
    assert_eq!(d1.to_string(), "1v0");
    let d2 = query.get(&world, e2).unwrap();
    // NameOrEntity Display for entities with a Name should be the Name
    assert_eq!(d2.to_string(), "MyName");
}
#[test]
fn test_name_hash_is_fixed() {
    let str = "foobar";
    assert_eq!(Name::from(str).pre_hash(), fixed_hash_one(str));
}

#[test]
fn test_serde_name() {
    let name = Name::new("MyComponent");
    assert_tokens(&name, &[Token::String("MyComponent")]);
}
