use std::{
    array, iter::Zip, ops::{Index, IndexMut},
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::ecs::{consts::SPARSE_PAGE_SIZE, id::Id};

pub mod entity_stroge;
mod test;

pub use entity_stroge::*;

pub struct SparsePos {
    pub page: usize,
    pub slot: usize,
}
impl SparsePos {
    #[inline(always)]
    pub fn new(page: usize, slot: usize) -> Self {
        Self { page, slot }
    }

    #[inline(always)]
    pub fn from_entity(entity: usize) -> Self {
        Self {
            page: entity / SPARSE_PAGE_SIZE,
            slot: entity % SPARSE_PAGE_SIZE,
        }
    }
}

#[derive(Debug)]
struct Page<T>(Box<[T; SPARSE_PAGE_SIZE]>)
where
    T: Id;
impl<T> Page<T>
where
    T: Id,
{
    pub fn new() -> Self {
        Self(Box::new(array::from_fn(|_| T::get_invalide_id())))
    }
}

#[derive(Debug)]
pub struct SparseSet<I, V>
where
    I: Id,
{
    dense_values: Vec<V>,
    dense_ids: Vec<I>,
    sparse: Vec<Page<I>>,
}
impl<I, V> SparseSet<I, V>
where
    I: Id,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense_values: Vec::with_capacity(capacity),
            dense_ids: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, id: &I, value: V) {
        let sparse_pos = Self::get_sparse_pos(id);
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        let sparse_value = &mut self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if sparse_value.is_avalide() {
            debug_assert!(
                sparse_value.get_version() == id.get_version(),
                "Try insert a invalide version id! id: {:?}",
                id
            );
            self.dense_values[sparse_value.get_idx() as usize] = value;
            self.dense_ids[sparse_value.get_idx() as usize] = id.clone();
            self.sparse[sparse_pos.page].0[sparse_pos.slot].replace_flags(id.get_flags());
        } else {
            let end = self.dense_values.len();
            self.dense_values.push(value);
            self.dense_ids.push(id.clone());

            *sparse_value = I::from_other(end as u32, &id);
        }
    }

    pub fn remove(&mut self, id: I) -> V {
        let sparse_pos = Self::get_sparse_pos(&id);
        debug_assert!(
            self.dense_values.len() > 0,
            "The dense array is empty while remove element! id: {:?}",
            id
        );
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "Remove the id is not alive! id: {:?}",
            id
        );

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            sparse_value == &id,
            "The id is invalided while remove the id! id: {:?}",
            id
        );

        let end_index = self.dense_values.len() - 1;
        let end_id = &self.dense_ids[end_index];
        let end_sparse_pos = Self::get_sparse_pos(end_id);
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];

        let index = sparse_value.get_idx() as usize;

        self.dense_values.swap(index, end_index);
        self.dense_ids.swap(index, end_index);

        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.get_idx(), &end_sparse_value);
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();

        self.dense_ids.pop();
        self.dense_values.pop().unwrap()
    }

    #[inline(always)]
    pub fn get_value(&self, id: &I) -> &V {
        &self[id]
    }

    #[inline(always)]
    pub fn get_value_mut(&mut self, id: &I) -> &mut V {
        &mut self[id]
    }

    #[inline(always)]
    pub fn has(&self, id: &I) -> bool {
        let sparse_pos = Self::get_sparse_pos(id);
        if self.sparse.get(sparse_pos.page).is_none() {
            return false;
        }
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if index.get_version() != id.get_version() {
            return false;
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide()
    }

    fn get_sparse_pos<T>(id: &T) -> SparsePos
    where
        T: Id,
    {
        let idx = id.get_idx() as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }
}

impl<I, V> Index<&I> for SparseSet<I, V>
where
    I: Id,
{
    type Output = V;

    fn index(&self, id: &I) -> &Self::Output {
        let sparse_pos = Self::get_sparse_pos(id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when get value, id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "The id is not alive! while get value, id: {:?}",
            id
        );
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &self.dense_values[index.get_idx() as usize]
    }
}

impl<I, V> IndexMut<&I> for SparseSet<I, V>
where
    I: Id,
{
    fn index_mut(&mut self, id: &I) -> &mut Self::Output {
        let sparse_pos = Self::get_sparse_pos(id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when get value, id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "The id is not alive! while get value, id: {:?}",
            id
        );
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            id.get_version() == index.get_version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &mut self.dense_values[index.get_idx() as usize]
    }
}

impl<I, V> SparseSet<I, V>
where
    I: Id,
{
    pub fn iter(&self) -> Zip<std::slice::Iter<'_, I>, std::slice::Iter<'_, V>> {
        self.dense_ids.iter().zip(&self.dense_values)
    }
    pub fn iter_mut(&mut self) -> Zip<std::slice::Iter<'_, I>, std::slice::IterMut<'_, V>> {
        self.dense_ids.iter().zip(&mut self.dense_values)
    }
}

/// ID 分配器，支持并发预留。
///
/// # 数据布局
///
/// ```text
/// dense:  [A, B, C, D, E, F, G, H]
///          ←── flushed ──→← free →
///          0       flushed_head  head    dense.len()
/// ```
///
/// - `dense[0..flushed_head]`: 已 flush 的活跃实体，sparse 中有对应条目
/// - `dense[flushed_head..head]`: 已预留但未 flush 的实体（来自 freelist），sparse 中仍为 INVALID
/// - `dense[head..]`: 空闲槽位，可被后续 remove 回收利用
/// - `head > dense.len()` 时：超出部分是全新实体，在 flush 时 push 到 dense
///
/// # 并发安全
///
/// - `reserve_entity(&self)` 只做原子操作，可多线程并发调用
/// - `flush(&mut self)` 将预留实体写回 sparse，需要独占引用
/// - `remove` 在释放时**立即递增版本号**存入 dense，保证 reserve 拿到的版本已正确
#[derive(Debug)]
pub struct SparseStroge<I>
where
    I: Id,
{
    dense: Vec<I>,
    sparse: Vec<Page<I>>,
    /// 已 flush 到 sparse 的实体数量（总是 ≤ head）
    flushed_head: usize,
    /// 已分配（含预留）的实体总数，原子变量支持并发预留
    head: AtomicUsize,
}

impl<I> SparseStroge<I>
where
    I: Id,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
            flushed_head: 0,
            head: AtomicUsize::new(0),
        }
    }

    /// 预留一个实体 ID（并发安全，只需要 `&self`）。
    ///
    /// 返回的实体句柄立即可用，但其 sparse 条目要等到 [`flush`](Self::flush) 后才会建立。
    /// 在 flush 之前，查询 sparse 的操作（如 `SparseSet::get_value`）对该实体无效。
    pub fn reserve_entity(&self) -> I {
        let old_head = self.head.fetch_add(1, Ordering::Relaxed);
        if old_head < self.dense.len() {
            // 复用 freelist 中的槽位：版本号已在 remove 时递增好
            self.dense[old_head].clone()
        } else {
            // 全新实体，version = 0
            I::new(old_head as u32, 0, I::FlagType::default())
        }
    }

    /// 批量预留实体 ID
    pub fn reserve_entities(&self, count: usize) -> Vec<I> {
        let old_head = self.head.fetch_add(count, Ordering::Relaxed);
        let dense_len = self.dense.len();
        (old_head..(old_head + count))
            .map(|i| {
                if i < dense_len {
                    self.dense[i].clone()
                } else {
                    I::new(i as u32, 0, I::FlagType::default())
                }
            })
            .collect()
    }

    /// 将所有通过 [`reserve_entity`](Self::reserve_entity) 预留的实体写入 sparse。
    ///
    /// 任何需要查询 sparse 的操作之前都应调用此方法。
    pub fn flush(&mut self) {
        let head = *self.head.get_mut();
        let dense_len = self.dense.len();

        // 1. 处理 freelist 中已被预留的实体（[flushed_head, min(head, dense_len))）
        //    这些实体的版本号已在 remove 时递增好，只需重建 sparse 条目
        let freelist_end = head.min(dense_len);
        for i in self.flushed_head..freelist_end {
            let entity = &self.dense[i];
            let sparse_pos = Self::get_sparse_pos(entity);
            debug_assert!(
                self.sparse.get(sparse_pos.page).is_some(),
                "flush freelist: no page for entity {:?}",
                entity
            );
            // sparse 条目指向 dense 中的位置 i
            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(i as u32, entity.get_version(), entity.get_flags());
        }

        // 2. 处理全新实体（dense_len..head），需要扩展 dense
        for i in dense_len..head {
            let entity = I::new(i as u32, 0, I::FlagType::default());
            let sparse_pos = Self::get_sparse_pos(&entity);
            if self.sparse.get(sparse_pos.page).is_none() {
                self.sparse.push(Page::new());
            }
            self.sparse[sparse_pos.page].0[sparse_pos.slot] = entity.clone();
            self.dense.push(entity);
        }

        self.flushed_head = head;
    }

    /// 是否需要 flush
    #[inline]
    pub fn needs_flush(&self) -> bool {
        self.flushed_head != self.head.load(Ordering::Relaxed)
    }

    /// 分配一个实体（单线程版本，内部调用 flush）。
    ///
    /// 等价于 `flush()` + 分配，用于不需要并发预留的场景。
    pub fn next(&mut self) -> I {
        self.flush();

        let head = *self.head.get_mut();
        let entity = if head < self.dense.len() {
            // 复用 freelist：版本号已在 remove() 中递增好，直接取用
            // 注意：不能再调 get_next_version，否则会双重递增版本号
            let entity = self.dense[head].clone();

            let sparse_pos = Self::get_sparse_pos(&entity);
            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(head as u32, entity.get_version(), entity.get_flags());
            entity
        } else {
            // 全新实体，version = 0
            let entity = I::new(head as u32, 0, I::FlagType::default());

            let sparse_pos = Self::get_sparse_pos(&entity);
            if self.sparse.get(sparse_pos.page).is_none() {
                self.sparse.push(Page::new());
            }
            self.sparse[sparse_pos.page].0[sparse_pos.slot] = entity.clone();
            self.dense.push(entity.clone());
            entity
        };

        self.flushed_head = head + 1;
        *self.head.get_mut() = head + 1;
        entity
    }

    /// 删除实体，将其回收到 freelist。
    ///
    /// **版本号在此时立即递增**，保证后续 `reserve_entity` 拿到的是新版本。
    pub fn remove(&mut self, id: I) {
        self.flush();

        let sparse_pos = Self::get_sparse_pos(&id);
        debug_assert!(
            self.sparse.get(sparse_pos.page).is_some(),
            "No page when remove id: {:?}",
            id
        );
        debug_assert!(
            self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide(),
            "Remove the id is not alive! id: {:?}",
            id
        );

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        debug_assert!(
            sparse_value.get_version() == id.get_version(),
            "The id's version is invalided while remove the id! id: {:?}",
            id
        );

        let head = *self.head.get_mut();
        let new_head = head - 1;
        let end_index = new_head; // 回收的目标位置
        let end_id = &self.dense[end_index];
        let end_sparse_pos = Self::get_sparse_pos(end_id);
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];

        let index = sparse_value.get_idx() as usize;

        self.dense.swap(index, end_index);

        // 更新被 swap 移动的实体的 sparse 条目
        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.get_idx(), &end_sparse_value);

        // 标记被删除实体的 sparse 为 INVALID
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();

        // ★ 关键：立即递增版本号存入 dense[end_index]，供后续 reserve_entity 使用
        let recycled = self.dense[end_index]
            .clone()
            .get_next_version(I::FlagType::default()); // 这里 flag 设为 default，通过 version 判断是否有效，避免 reserve 时需要 mut 改变Flag
        self.dense[end_index] = recycled;

        *self.head.get_mut() = new_head;
        self.flushed_head = new_head;
    }

    /// 检查实体是否存在。
    ///
    /// 注意：已通过 `reserve_entity` 预留但未 flush 的实体也返回 `true`。
    #[inline(always)]
    pub fn has(&self, id: I) -> bool {
        let idx = id.get_idx() as usize;
        let dense_len = self.dense.len();

        if idx < dense_len {
            // 可能在 dense 中：检查 sparse 条目
            let sparse_pos = Self::get_sparse_pos(&id);
            if self.sparse.get(sparse_pos.page).is_none() {
                return false;
            }
            let sparse_val = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
            if sparse_val.get_version() == id.get_version() && sparse_val.is_avalide() {
                return true;
            }

            // 也可能在 reserved 区（sparse 尚未更新但 dense 中已分配）
            let head = self.head.load(Ordering::Relaxed);
            if idx < head && idx >= self.flushed_head {
                let dense_val = &self.dense[idx];
                return dense_val.get_idx() == id.get_idx()
                    && dense_val.get_version() == id.get_version();
            }
            false
        } else {
            // 超出 dense：检查是否在 reserved 全新实体范围
            let head = self.head.load(Ordering::Relaxed);
            idx < head && id.get_version() == 0
        }
    }

    /// 当前活跃实体总数（含已预留未 flush）
    #[inline]
    pub fn len(&self) -> usize {
        self.head.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get_sparse_pos<T>(id: &T) -> SparsePos
    where
        T: Id,
    {
        let idx = id.get_idx() as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }
}
