use std::hash::{BuildHasher, Hasher};

use crate::{
    ecs::intern::{Internable, Interned, Interner},
    hash::FixedHasher,
};

#[test]
fn zero_sized_type() {
    #[derive(PartialEq, Eq, Hash, Debug)]
    pub struct A;

    impl Internable for A {
        fn leak(&self) -> &'static Self {
            &A
        }

        fn ref_eq(&self, other: &Self) -> bool {
            core::ptr::eq(self, other)
        }

        fn ref_hash<H: Hasher>(&self, state: &mut H) {
            core::ptr::hash(self, state);
        }
    }

    let interner = Interner::default();
    let x = interner.intern(&A);
    let y = interner.intern(&A);
    assert_eq!(x, y);
}

#[test]
fn fieldless_enum() {
    #[derive(PartialEq, Eq, Hash, Debug, Clone)]
    pub enum A {
        X,
        Y,
    }

    impl Internable for A {
        fn leak(&self) -> &'static Self {
            match self {
                A::X => &A::X,
                A::Y => &A::Y,
            }
        }

        fn ref_eq(&self, other: &Self) -> bool {
            core::ptr::eq(self, other)
        }

        fn ref_hash<H: Hasher>(&self, state: &mut H) {
            core::ptr::hash(self, state);
        }
    }

    let interner = Interner::default();
    let x = interner.intern(&A::X);
    let y = interner.intern(&A::Y);
    assert_ne!(x, y);
}

#[test]
fn static_sub_strings() {
    let str = "ABC ABC";
    let a = &str[0..3];
    let b = &str[4..7];
    // Same contents
    assert_eq!(a, b);
    let x = Interned(a);
    let y = Interned(b);
    // Different pointers
    assert_ne!(x, y);
    let interner = Interner::default();
    let x = interner.intern(a);
    let y = interner.intern(b);
    // Same pointers returned by interner
    assert_eq!(x, y);
}

#[test]
fn same_interned_instance() {
    let a = Interned("A");
    let b = a;

    assert_eq!(a, b);

    let hash_a = FixedHasher.hash_one(a);
    let hash_b = FixedHasher.hash_one(b);

    assert_eq!(hash_a, hash_b);
}

#[test]
fn same_interned_content() {
    let a = Interned::<str>(Box::leak(Box::new("A".to_string())));
    let b = Interned::<str>(Box::leak(Box::new("A".to_string())));

    assert_ne!(a, b);
}

#[test]
fn different_interned_content() {
    let a = Interned::<str>("A");
    let b = Interned::<str>("B");

    assert_ne!(a, b);
}
