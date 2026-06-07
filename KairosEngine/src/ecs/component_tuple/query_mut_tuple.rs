use std::ptr;

use crate::ecs::{
    compoent_register::ComponentRegister, component::Component, table_graph::TableGraph,
};

pub trait ComponentQueryMutTuple {
    type Item<'a>
    where
        Self: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a mut TableGraph, f: F)
    where
        F: FnMut(Self::Item<'a>),
        Self: 'a;
}

impl<A: Component, B: Component> ComponentQueryMutTuple for (A, B) {
    type Item<'a>
        = (&'a A, &'a B)
    where
        A: 'a,
        B: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a mut TableGraph, mut f: F)
    where
        F: FnMut(Self::Item<'a>),
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

        for table in table_graph.graph.node_weights_mut() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let (a, b) = unsafe {
                let table = ptr::from_mut(table);
                let a = (*table).component_slice_mut::<A>(&a_component_id);
                let b = (*table).component_slice_mut::<B>(&b_component_id);
                (a, b)
            };

            a.iter_mut().zip(b).for_each(|(a, b)| f((a, b)));
        }
    }
}
impl<A: Component, B: Component, C: Component> ComponentQueryMutTuple for (A, B, C) {
    type Item<'a>
        = (&'a A, &'a B, &'a C)
    where
        A: 'a,
        B: 'a,
        C: 'a;

    fn foreach<'a, F>(register: &mut ComponentRegister, table_graph: &'a mut TableGraph, mut f: F)
    where
        F: FnMut(Self::Item<'a>),
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

        for table in table_graph.graph.node_weights_mut() {
            if !table.contains_all_components(&component_ids) {
                continue;
            }

            let (a, b, c) = unsafe {
                let table = ptr::from_mut(table);
                let a = (*table).component_slice_mut::<A>(&a_component_id);
                let b = (*table).component_slice_mut::<B>(&b_component_id);
                let c = (*table).component_slice_mut::<C>(&c_component_id);
                (a, b, c)
            };

            a.iter_mut()
                .zip(b)
                .zip(c)
                .map(|((a, b), c)| (a, b, c))
                .for_each(|(a, b, c)| f((a, b, c)));
        }
    }
}
