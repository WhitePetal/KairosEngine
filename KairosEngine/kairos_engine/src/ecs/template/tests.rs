// use crate::ecs::world::World;

// #[test]
// fn option_template() {
//     #[derive(FromTemplate)]
//     struct Handle(String);

//     #[derive(FromTemplate)]
//     struct Foo {
//         #[template(built_in)]
//         handle: Option<Handle>,
//     }

//     let mut world = World::new();
//     let foo_template = FooTemplate {
//         handle: Some(HandleTemplate("handle_path".to_string())).into(),
//     };
//     let foo = world.spawn_empty().build_template(&foo_template).unwrap();
//     assert_eq!(foo.handle.unwrap().0, "handle_path".to_string());
// }
