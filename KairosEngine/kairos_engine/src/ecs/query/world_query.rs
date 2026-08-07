pub unsafe trait WorldQuery {
    type Fetch<'w>: Clone;

    type State: Send + Sync + Sized;
}

// TODO!
