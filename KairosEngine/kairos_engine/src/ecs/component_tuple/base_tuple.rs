use std::{any::TypeId, hash::Hash, ptr::NonNull};

use kairos_ecs_macros::ComponentTuple;

use crate::ecs::{
    component::{Component, MissingComponent},
    table::ComponentTypeInfo,
};

type ComponentWriter = Box<dyn FnOnce(*mut u8)>;

#[derive(Debug, PartialEq, Eq)]
pub struct ComponentTupleKey(TypeId);
impl Hash for ComponentTupleKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let data =
        // SAFETY: The `offset` stays in-bounds, it just moves the pointer to the 2nd half of the `TypeId`.
        // Only the first ptr-sized chunk ever has provenance, so that second half is always
        // fine to read at integer type.
            unsafe { std::mem::transmute_copy(&self.0) };
        state.write_u64(data);
    }
}
impl From<TypeId> for ComponentTupleKey {
    fn from(value: TypeId) -> Self {
        Self(value)
    }
}

pub unsafe trait DynamicComponentTuple {
    fn has<T: Component>(&self) -> bool;

    fn key(&self) -> Option<ComponentTupleKey>;

    fn type_infos(&self) -> Box<[ComponentTypeInfo]>;

    unsafe fn put<F: FnMut(*mut u8, ComponentTypeInfo)>(self, f: F);

    fn with_ids<T, F: FnOnce(&[TypeId]) -> T>(&self, f: F) -> T;
}

/// 编译期能够确定的，静态类型组件Tuple
pub unsafe trait ComponentTuple: DynamicComponentTuple {
    fn with_static_ids<T, F: FnOnce(&[TypeId]) -> T>(f: F) -> T;

    fn with_static_type_info<T, F: FnOnce(&[ComponentTypeInfo]) -> T>(f: F) -> T;

    unsafe fn get<F: FnMut(ComponentTypeInfo) -> Option<NonNull<u8>>>(
        f: F,
    ) -> Result<Self, MissingComponent>
    where
        Self: Sized;
}


macro_rules! count {
    () => { 0 };
    ($x: ident $(, $rest: ident)*) => { 1 + count!($($rest),*) };
}

macro_rules! tuple_impl {
    ($($name: ident), *) => {
        unsafe impl<$($name: Component), *> DynamicComponentTuple for ($($name,)*) {
            #[allow(non_camel_case_types)]
            fn has<__ecs__T: Component>(&self) -> bool {
                false $(|| TypeId::of::<$name>() == TypeId::of::<__ecs__T>())*
            }

            fn key(&self) -> Option<ComponentTupleKey> {
                Some(ComponentTupleKey::from(TypeId::of::<Self>()))
            }

            #[allow(non_camel_case_types)]
            fn with_ids<__ecs_T, __ecs__F: FnOnce(&[TypeId]) -> __ecs_T>(&self, f: __ecs__F) -> __ecs_T {
                Self::with_static_ids(f)
            }

            fn type_infos(&self) -> Box<[ComponentTypeInfo]> {
                Self::with_static_type_info(|info| info.iter().copied().collect::<Box<[_]>>())
            }

            #[allow(unused_variables, unused_mut, non_camel_case_types)]
            unsafe fn put<__ecs__F: FnMut(*mut u8, ComponentTypeInfo)>(self, mut f: __ecs__F) {
                #[allow(non_snake_case)]
                let ($(mut $name,)*) = self;
                $(
                    f(
                        (&mut $name as *mut $name).cast::<u8>(),
                        ComponentTypeInfo::of::<$name>()
                    );
                    std::mem::forget($name);
                )*
            }
        }

        unsafe impl<$($name: Component), *> ComponentTuple for ($($name,)*) {
            #[allow(non_camel_case_types)]
            fn with_static_ids<__ecs__T, __ecs__F: FnOnce(&[TypeId]) -> __ecs__T>(f: __ecs__F) -> __ecs__T {
                const N: usize = count!($($name),*);
                let mut xs: [(usize, TypeId); N] = [$((std::mem::align_of::<$name>(), TypeId::of::<$name>())),*];
                xs.sort_unstable_by(|x, y| {
                    x.0.cmp(&y.0).reverse().then(x.1.cmp(&y.1))
                });
                let mut ids = [TypeId::of::<()>(); N];
                for (slot, &(_, id)) in ids.iter_mut().zip(xs.iter()) {
                    *slot = id;
                }
                f(&ids)
            }

            #[allow(non_camel_case_types)]
            fn with_static_type_info<__ecs__T, __ecs__F: FnOnce(&[ComponentTypeInfo]) -> __ecs__T>(f: __ecs__F) -> __ecs__T {
                const N: usize = count!($($name),*);
                let mut xs: [ComponentTypeInfo; N] = [$(ComponentTypeInfo::of::<$name>()),*];
                xs.sort_unstable();
                f(&xs)
            }

            #[allow(unused_variables, unused_mut, non_camel_case_types, unsafe_op_in_unsafe_fn)]
            unsafe fn get<__ecs__F: FnMut(ComponentTypeInfo) -> Option<NonNull<u8>>>(mut f: __ecs__F) -> Result<Self, MissingComponent> {
                #[allow(non_snake_case)]
                let ($(mut $name,)*) = ($(
                    f(ComponentTypeInfo::of::<$name>()).ok_or_else(MissingComponent::new::<$name>)?
                        .as_ptr()
                        .cast::<$name>(),)*
                );
                Ok(($($name.read(),)*))
            }
        }
    };
}

macro_rules! reverse_apply {
    ($m: ident [] $($reversed:tt)*) => {
        $m!{$($reversed),*}  // base case
    };
    ($m: ident [$first:tt $($rest:tt)*] $($reversed:tt)*) => {
        reverse_apply!{$m [$($rest)*] $first $($reversed)*}
    };
}

macro_rules! smaller_tuples_too {
    ($m: ident, $next: tt) => {
        $m!{}
        $m!{$next}
    };
    ($m: ident, $next: tt, $($rest: tt),*) => {
        smaller_tuples_too!{$m, $($rest),*}
        reverse_apply!{$m [$next $($rest)*]}
    };
}

smaller_tuples_too!(tuple_impl, O, N, M, L, K, J, I, H, G, F, E, D, C, B, A);


// struct Position {}
// impl Component for Position {}
// struct Velocity {}
// impl Component for Velocity {}
// struct Health {}
// impl Component for Health {}

// #[derive(ComponentTuple)]
// struct PlayerTuple {
//     position: Position,
//     velocity: Velocity,
//     health: Health,
// }

// impl DynamicComponentTuple for PlayerTuple {
//     fn has<T: Component>(&self) -> bool {
//         false
//             || core::any::TypeId::of::<Position>() == core::any::TypeId::of::<T>()
//             || core::any::TypeId::of::<Velocity>() == core::any::TypeId::of::<T>()
//             || core::any::TypeId::of::<Health>() == core::any::TypeId::of::<T>()
//     }

//     fn key(&self) -> Option<ComponentTupleKey> {
//         core::option::Option::Some(ComponentTupleKey::from(core::any::TypeId::of::<Self>()))
//     }

//     fn type_infos(&self) -> Box<[ComponentTypeInfo]> {
//         <Self as ComponentTuple>::with_static_type_info(|info| {
//             info.iter().copied().collect::<Box<[_]>>()
//         })
//     }

//     unsafe fn put<F: FnMut(*mut u8, ComponentTypeInfo)>(mut self, mut f: F) {
//         f(
//             (&mut self.position as *mut Position).cast::<u8>(),
//             ComponentTypeInfo::of::<Position>(),
//         );
//         core::mem::forget(self.position);

//         f(
//             (&mut self.velocity as *mut Velocity).cast::<u8>(),
//             ComponentTypeInfo::of::<Velocity>(),
//         );

//         f(
//             (&mut self.health as *mut Health).cast::<u8>(),
//             ComponentTypeInfo::of::<Velocity>(),
//         );
//     }

//     fn with_ids<T, F: FnOnce(&[TypeId]) -> T>(&self, f: F) -> T {
//         <Self as ComponentTuple>::with_static_ids(f)
//     }
// }

// struct EnemyTag {}

// impl DynamicComponentTuple for EnemyTag {
//     fn has<T: Component>(&self) -> bool {
//         false
//     }

//     fn key(&self) -> Option<ComponentTupleKey> {
//         core::option::Option::Some(ComponentTupleKey::from(core::any::TypeId::of::<Self>()))
//     }

//     fn type_infos(&self) -> Box<[ComponentTypeInfo]> {
//         <Self as ComponentTuple>::with_static_type_info(|info| {
//             info.iter().copied().collect::<Box<[_]>>()
//         })
//     }

//     unsafe fn put<F: FnMut(*mut u8, ComponentTypeInfo)>(mut self, mut f: F) {

//     }

//     fn with_ids<T, F: FnOnce(&[TypeId]) -> T>(&self, f: F) -> T {
//         <Self as ComponentTuple>::with_static_ids(f)
//     }
// }

// impl ComponentTuple for EnemyTag {
//     // tag 这类空组件，单独存在时会都存入根表中(里面的实体都是身上没组件的实体)
//     // 因此无法通过单个tag来进行查找，因为所有tag都被视为空组件存在根表中
//     // tag必须与其它组件绑定在实体上，这样能够被转移到其它表中，从而可以进行查找等操作
//     fn with_static_ids<T, F: FnOnce(&[TypeId]) -> T>(f: F) -> T {
//         f(&[])
//     }

//     fn with_static_type_info<T, F: FnOnce(&[ComponentTypeInfo]) -> T>(f: F) -> T {
//         f(&[])
//     }

//     unsafe fn get<F: FnMut(ComponentTypeInfo) -> Option<NonNull<u8>>>(
//         f: F,
//     ) -> Result<Self, MissingComponent>
//     where
//         Self: Sized,
//     {
//         Ok(Self {})
//     }
// }
