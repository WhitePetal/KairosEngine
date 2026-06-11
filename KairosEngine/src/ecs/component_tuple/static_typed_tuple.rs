use std::{any::TypeId, ptr::NonNull};

use crate::ecs::{
    component::MissingComponent, component_tuple::ComponentTuple, table::ComponentTypeInfo,
};

/// 编译期能够确定的，静态类型组件Tuple
pub trait StaticTypedComponentTuple: ComponentTuple {
    fn with_static_ids<T, F: FnOnce(&[TypeId]) -> T>(f: F) -> T;

    fn with_static_type_info<T, F: FnOnce(&[ComponentTypeInfo]) -> T>(f: F) -> T;

    fn get<F: FnMut(ComponentTypeInfo) -> Option<NonNull<u8>>>(
        f: F,
    ) -> Result<Self, MissingComponent>
    where
        Self: Sized;
}
