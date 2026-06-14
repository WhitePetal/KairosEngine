use std::{any::TypeId, ptr::NonNull};

use crate::ecs::{entity::Entity, table::Table, world::EntityData};


/// [`Query`] 对 [`Table`] 的访问类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Access {
    /// 只读取实体Id，不读取组件
    Iterate,
    /// 读取组件
    Read,
    /// 读写组件
    Write
}

/// 组件列上的流式迭代器
#[allow(clippy::missing_safety_doc)]
pub unsafe trait Fetch: Clone + Sized + 'static {
    /// Table 中 Fetch 需要访问的特定列的运行时关联状态
    type State: Copy + Send + Sync;

    /// 对于不需要 `get` 的 query，返回的占位值
    fn dangling() -> Self;

    /// 定义Query如何访问Table
    fn access(table: &Table) -> Option<Access>;

    /// 从`table`获取动态借用
    fn borrow(table: &Table, state: Self::State);
    /// 遍历`table`前，先查询它的状态
    fn prepare(table: &Table) -> Option<Self::State>;
    /// 基于关联状态给`table`构造`Fetch`
    fn execute(table: &Table, state: Self::State) -> Self;
    /// 释放从`borrow`获取的动态借用
    fn release(table: &Table, state: Self::State);

    /// 用于编译期检查，检查Fetch中的所有借用关系
    fn for_each_borrow<F: FnMut(TypeId, bool)>(f: F);
}

/// 从[`World`](crate::ecs::world::World)获取的组件类型集合
pub trait Query {
    /// 查询的结果类型
    type Item<'a>;

    type Fetch: Fetch;

    /// 获取Table中第n行的数据
    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a>;
}

/// 用于标记 Item 不会产出 &mut T 的 Query
#[allow(clippy::missing_safety_doc)]
pub unsafe trait QueryShared {}

#[derive(Clone)]
pub struct FetchEntity(NonNull<Entity>);
unsafe impl Fetch for FetchEntity {
    type State = ();

    fn dangling() -> Self {
        todo!()
    }

    fn access(table: &Table) -> Option<Access> {
        todo!()
    }

    fn borrow(table: &Table, state: Self::State) {
        todo!()
    }

    fn prepare(table: &Table) -> Option<Self::State> {
        todo!()
    }

    fn execute(table: &Table, state: Self::State) -> Self {
        Self(table.entities())
    }

    fn release(table: &Table, state: Self::State) {
        todo!()
    }

    fn for_each_borrow<F: FnMut(TypeId, bool)>(f: F) {
        todo!()
    }
}

impl Query for EntityData {
    type Item<'a> = Entity;
    type Fetch = FetchEntity;
    
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a> {
        let entity = fetch.0.as_ptr().add(n).read();
        entity
    }

    
}