

pub enum Drag<T> {
    Draging(T),
    Stoped(T),
}

impl<T> Drag<T> {
    pub fn get(&self) -> &T {
        match self {
            Drag::Draging(value) => value,
            Drag::Stoped(value) => value,
        }
    }
}
