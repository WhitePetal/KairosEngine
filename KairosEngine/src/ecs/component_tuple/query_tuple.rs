use crate::ecs::{
    compoent_register::ComponentRegister, component::Component, table_graph::TableGraph,
};

pub trait ComponentQueryTuple {
    type Item<'a>
    where
        Self: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a;
}

impl<A: Component> ComponentQueryTuple for A {
    type Item<'a>
        = (&'a A)
    where
        Self: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a,
    {
        let a_component_id = register.get::<A>().0;
        let component_ids = [&a_component_id];

        for table in table_graph.graph.node_weights() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let a_components = table.component_slice::<A>(&component_ids[0]);
            a_components.iter().for_each(|a| f(a));
        }
    }
}
impl<A: Component, B: Component> ComponentQueryTuple for (A, B) {
    type Item<'a>
        = (&'a A, &'a B)
    where
        A: 'a,
        B: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a,
    {
        let a_component_id = register.get::<A>().0;
        let b_component_id = register.get::<B>().0;
        let mut component_ids = [&a_component_id, &b_component_id];
        component_ids.sort();

        debug_assert!(
            { component_ids.windows(2).all(|pair| pair[0] != pair[1]) },
            "Query cant contain the same component type!, component_ids: {:?}",
            component_ids
        );

        for table in table_graph.graph.node_weights() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let a_components = table.component_slice::<A>(&a_component_id);
            let b_components = table.component_slice::<B>(&b_component_id);

            a_components
                .iter()
                .zip(b_components)
                .for_each(|(a, b)| f((a, b)));
        }
    }
}
impl<A: Component, B: Component, C: Component> ComponentQueryTuple for (A, B, C) {
    type Item<'a>
        = (&'a A, &'a B, &'a C)
    where
        A: 'a,
        B: 'a,
        C: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a TableGraph, f: F)
    where
        F: Fn(Self::Item<'a>),
        Self: 'a,
    {
        let a_component_id = register.get::<A>().0;
        let b_component_id = register.get::<B>().0;
        let c_component_id = register.get::<C>().0;
        let mut component_ids = [&a_component_id, &b_component_id, &c_component_id];
        component_ids.sort();

        debug_assert!(
            { component_ids.windows(2).all(|pair| pair[0] != pair[1]) },
            "Query cant contain the same component type!, component_ids: {:?}",
            component_ids
        );

        for table in table_graph.graph.node_weights() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let a_components = table.component_slice::<A>(&a_component_id);
            let b_components = table.component_slice::<B>(&b_component_id);
            let c_components = table.component_slice::<C>(&c_component_id);

            a_components
                .iter()
                .zip(b_components)
                .zip(c_components)
                .for_each(|((a, b), c)| f((a, b, c)));
        }
    }
}
