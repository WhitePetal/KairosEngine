use std::{
    any::Any,
    hash::{Hash, Hasher},
};

#[doc(hidden)]
pub use Box;

pub trait DynEq: Any {
    fn dyn_eq(&self, other: &dyn DynEq) -> bool;
}
impl<T> DynEq for T
where
    T: Any + Eq,
{
    fn dyn_eq(&self, other: &dyn DynEq) -> bool {
        if let Some(other) = (other as &dyn Any).downcast_ref::<T>() {
            return self == other;
        }
        false
    }
}

pub trait DynHash: DynEq {
    fn dyn_hash(&self, state: &mut dyn Hasher);
}
impl<T> DynHash for T
where
    T: DynEq + Hash,
{
    fn dyn_hash(&self, mut state: &mut dyn Hasher) {
        T::hash(self, &mut state);
        self.type_id().hash(&mut state);
    }
}

#[macro_export]
macro_rules! define_label {
    (
        $(#[$label_attr:meta])*
        $label_trait_name:ident,
        $interner_name:ident
    ) => {
        $crate::define_label!(
            $(#[$label_attr])*
            $label_trait_name,
            $interner_name,
            extra_methods: {},
            extra_methods_impl: {}
        );
    };
    (
        $(#[$label_attr:meta])*
        $label_trait_name:ident,
        $interner_name:ident,
        extra_methods: { $($trait_extra_methods:tt)* },
        extra_methods_impl: { $($interned_extra_methods_impl:tt)* }
    ) => {

        $(#[$label_attr])*
        pub trait $label_trait_name: Send + Sync + ::core::fmt::Debug + $crate::ecs::label::DynEq + $crate::ecs::label::DynHash {

            $($trait_extra_methods)*

            #[doc = stringify!($label_trait_name)]
            fn dyn_clone(&self) -> $crate::ecs::label::Box<dyn $label_trait_name>;

            fn intern(&self) -> $crate::ecs::intern::Interned<dyn $label_trait_name>
            where Self: Sized {
                $interner_name.intern(self)
            }
        }

        #[diagnostic::do_not_recommend]
        impl $label_trait_name for $crate::ecs::intern::Interned<dyn $label_trait_name> {

            $($interned_extra_methods_impl)*

            fn dyn_clone(&self) -> $crate::ecs::label::Box<dyn $label_trait_name> {
                (**self).dyn_clone()
            }

            fn intern(&self) -> Self {
                *self
            }
        }

        impl PartialEq for dyn $label_trait_name {
            fn eq(&self, other: &Self) -> bool {
                self.dyn_eq(other)
            }
        }

        impl Eq for dyn $label_trait_name {}

        impl ::core::hash::Hash for dyn $label_trait_name {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.dyn_hash(state);
            }
        }

        impl $crate::ecs::intern::Internable for dyn $label_trait_name {
            fn leak(&self) -> &'static Self {
                $crate::ecs::label::Box::leak(self.dyn_clone())
            }

            fn ref_eq(&self, other: &Self) -> bool {
                use ::core::ptr;

                self.type_id() == other.type_id()
                    && ptr::addr_eq(ptr::from_ref::<Self>(self), ptr::from_ref::<Self>(other))
            }

            fn ref_hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                use ::core::{hash::Hash, ptr};

                self.type_id().hash(state);

                ptr::from_ref::<Self>(self).cast::<()>().hash(state);
            }
        }

        static $interner_name: $crate::ecs::intern::Interner<dyn $label_trait_name> =
            $crate::ecs::intern::Interner::new();
    }
}
