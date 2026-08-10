use std::marker::PhantomData;

pub struct Query<'world, 'state, D, F> {
    _todo: PhantomData<(&'world D, &'state F)>,
}

// TODO!
