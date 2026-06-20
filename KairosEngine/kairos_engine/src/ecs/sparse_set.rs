use core::error;
use std::{
    array, fmt,
    ops::{Index, IndexMut, Range},
    slice::Iter,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::ecs::{
    consts::SPARSE_PAGE_SIZE,
    id::{Id, IdFlag},
};

pub mod entity_stroge;
#[cfg(test)]
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
    sparse: Vec<Page<I>>,
}
impl<I, V> SparseSet<I, V>
where
    I: Id,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            dense_values: Vec::with_capacity(capacity),
            sparse: Vec::with_capacity(capacity),
        }
    }

    pub fn insert(&mut self, id: &I, value: V) {
        let sparse_pos = Self::get_sparse_pos(id.idx());
        if self.sparse.get(sparse_pos.page).is_none() {
            self.sparse.push(Page::new());
        }
        let sparse_value = &mut self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if sparse_value.is_avalide() {
            debug_assert!(
                sparse_value.version() == id.version(),
                "Try insert a invalide version id! id: {:?}",
                id
            );
            self.dense_values[sparse_value.idx() as usize] = value;
            self.sparse[sparse_pos.page].0[sparse_pos.slot].replace_flags(id.flags());
        } else {
            let end = self.dense_values.len();
            self.dense_values.push(value);

            *sparse_value = I::from_other(end as u32, &id);
        }
    }

    pub fn remove(&mut self, id: I, moved_id: I) -> Option<V> {
        let sparse_pos = Self::get_sparse_pos(id.idx());
        let sparse_value = self.sparse.get(sparse_pos.page)?.0.get(sparse_pos.slot)?;

        let end_sparse_pos = Self::get_sparse_pos(moved_id.idx());
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];
        let end_index = end_sparse_value.idx() as usize;

        let index = sparse_value.idx() as usize;

        self.dense_values.swap(index, end_index);

        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.idx(), &end_sparse_value);
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();

        self.dense_values.pop()
    }

    /// 直接扩容 additional 个单位
    pub fn reserve(&mut self, additional: (usize, Option<usize>)) {
        self.dense_values.reserve(additional.0);
        if let Some(page_shorfall) = additional.1 {
            self.sparse.reserve(page_shorfall);
            for _ in 0..page_shorfall {
                self.sparse.push(Page::new());
            }
        }
    }

    pub fn clear(&mut self) {
        self.dense_values.clear();
        for page in &mut self.sparse {
            page.0.fill(I::get_invalide_id());
        }
    }

    pub fn get(&self, entity: &I) -> Option<&V> {
        let sparse_pos = Self::get_sparse_pos(entity.idx());
        let index = self.sparse.get(sparse_pos.page)?.0.get(sparse_pos.slot)?;
        if index.version() != entity.version() {
            return None;
        }
        self.dense_values.get(index.idx() as usize)
    }

    pub fn get_mut(&mut self, entity: &I) -> Option<&mut V> {
        let sparse_pos = Self::get_sparse_pos(entity.idx());
        let index = self.sparse.get(sparse_pos.page)?.0.get(sparse_pos.slot)?;
        if index.version() != entity.version() {
            return None;
        }
        self.dense_values.get_mut(index.idx() as usize)
    }

    #[inline(always)]
    pub unsafe fn get_unchecked(&self, id: &I) -> &V {
        &self[id]
    }

    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, id: &I) -> &mut V {
        &mut self[id]
    }

    #[inline(always)]
    pub fn has(&self, id: &I) -> bool {
        let sparse_pos = Self::get_sparse_pos(id.idx());
        if self.sparse.get(sparse_pos.page).is_none() {
            return false;
        }
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if index.version() != id.version() {
            return false;
        }
        self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide()
    }

    fn get_sparse_pos(idx: u32) -> SparsePos {
        let idx = idx as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }

    pub fn len(&self) -> usize {
        self.dense_values.len()
    }
}

impl<I, V> Index<&I> for SparseSet<I, V>
where
    I: Id,
{
    type Output = V;

    fn index(&self, id: &I) -> &Self::Output {
        let sparse_pos = Self::get_sparse_pos(id.idx());
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        &self.dense_values[index.idx() as usize]
    }
}

impl<I, V> IndexMut<&I> for SparseSet<I, V>
where
    I: Id,
{
    fn index_mut(&mut self, id: &I) -> &mut Self::Output {
        let sparse_pos = Self::get_sparse_pos(id.idx());
        let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        &mut self.dense_values[index.idx() as usize]
    }
}

impl<I, V> Default for SparseSet<I, V>
where
    I: Id,
{
    fn default() -> Self {
        Self {
            dense_values: Default::default(),
            sparse: Default::default(),
        }
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

pub enum AllocAt<I>
where
    I: Id,
{
    New(Range<usize>),
    BeUsed(I),
    Using,
}

#[derive(Debug, Clone)]
pub struct AllocManyState {
    pub pending_range: Range<usize>,
    pub fresh: Range<usize>,
}
impl AllocManyState {
    pub fn next(&mut self) -> Option<usize> {
        if let Some(pending) = self.pending_range.next() {
            Some(pending)
        } else {
            self.fresh.next()
        }
    }

    pub fn len(&self) -> usize {
        self.fresh.len() + self.pending_range.len()
    }
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
    /// 在 flush 之前，查询 sparse 的操作（如 `SparseSet::get`）对该实体无效。
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
    /// return: flush_ids slice
    fn flush_inner(&mut self) -> &[I] {
        let head = *self.head.get_mut();
        let dense_len = self.dense.len();

        // 1. 处理 freelist 中已被预留的实体（[flushed_head, min(head, dense_len))）
        //    这些实体的版本号已在 remove 时递增好，只需重建 sparse 条目
        let freelist_end = head.min(dense_len);
        for i in self.flushed_head..freelist_end {
            let entity = &self.dense[i];
            let sparse_pos = Self::get_sparse_pos(entity.idx());
            debug_assert!(
                self.sparse.get(sparse_pos.page).is_some(),
                "flush freelist: no page for entity {:?}",
                entity
            );
            // sparse 条目指向 dense 中的位置 i
            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(i as u32, entity.version(), entity.flags());
        }

        // 2. 处理全新实体（dense_len..head），需要扩展 dense
        for i in dense_len..head {
            let entity = I::new(i as u32, 0, I::FlagType::default());
            let sparse_pos = Self::get_sparse_pos(entity.idx());
            if self.sparse.get(sparse_pos.page).is_none() {
                self.sparse.push(Page::new());
            }
            self.sparse[sparse_pos.page].0[sparse_pos.slot] = entity.clone();
            self.dense.push(entity);
        }

        let flushs = &self.dense[self.flushed_head..head];
        self.flushed_head = head;
        flushs
    }

    pub fn flush<F: FnMut(&I) -> ()>(&mut self, mut flush_fn: F) {
        let flushs = self.flush_inner();
        flushs.iter().for_each(|entity| {
            (flush_fn)(entity);
        });
    }

    /// 是否需要 flush
    #[inline]
    pub fn needs_flush(&self) -> bool {
        self.flushed_head != self.head.load(Ordering::Relaxed)
    }

    fn verify_flushed(&mut self) {
        debug_assert!(
            !self.needs_flush(),
            "flush() needs to be called before this operation is legal"
        )
    }

    /// 分配一个实体（单线程版本，内部调用 flush）。
    ///
    /// 等价于 `flush()` + 分配，用于不需要并发预留的场景。
    pub fn alloc(&mut self) -> I {
        self.verify_flushed();

        let head = *self.head.get_mut();
        let entity = {
            if head < self.dense.len() {
                // 复用 freelist：版本号已在 remove() 中递增好，直接取用
                // 注意：不能再调 get_next_version，否则会双重递增版本号
                let entity = self.dense[head].clone();

                let sparse_pos = Self::get_sparse_pos(entity.idx());
                self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                    I::new(head as u32, entity.version(), entity.flags());
                entity
            } else {
                // 全新实体，version = 0
                let entity = I::new(head as u32, 0, I::FlagType::default());

                let sparse_pos = Self::get_sparse_pos(entity.idx());
                if self.sparse.get(sparse_pos.page).is_none() {
                    self.sparse.push(Page::new());
                }
                self.sparse[sparse_pos.page].0[sparse_pos.slot] = entity.clone();
                self.dense.push(entity.clone());
                entity
            }
        };

        self.flushed_head = head + 1;
        *self.head.get_mut() = head + 1;
        entity
    }

    /// 分配指定的Entity
    ///
    /// 如果Entity不存在，则创建该Entity
    ///
    /// 否则用输入的Entity覆盖
    pub fn alloc_at(&mut self, entity: &I) -> AllocAt<I> {
        let idx = entity.idx() as usize;
        // Id 从未被创建过
        if idx as usize >= self.dense.len() {
            self.dense.extend(
                (self.dense.len()..idx)
                    .map(|i| I::new(i as u32, 0, I::FlagType::get_invalide_flag())),
            );
            let head = *self.head.get_mut();
            self.dense.swap(head, idx);
            self.dense[head] = entity.clone();
            let sparse_pos = Self::get_sparse_pos(entity.idx());
            if self.sparse.get(sparse_pos.page).is_none() {
                self.sparse.push(Page::new());
            }
            self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::from_other(head as u32, &entity);
            self.flushed_head = head + 1;
            *self.head.get_mut() = self.flushed_head;
            AllocAt::New(head..self.flushed_head)
        } else {
            let sparse_pos = Self::get_sparse_pos(entity.idx());
            // Id 被创建过，但被销毁了
            let sparse = &mut self.sparse[sparse_pos.page].0[sparse_pos.slot];
            let index = sparse.idx() as usize;
            if !sparse.is_avalide() {
                let head = *self.head.get_mut();
                let mut entity = entity.clone();
                entity.replace_flags(I::FlagType::default());
                self.dense[index] = entity.clone();
                self.swap_inner(head, index);
                self.flushed_head = head;
                *self.head.get_mut() = self.flushed_head;
                AllocAt::BeUsed(entity)
            } else {
                // Id 被创建过，但正在被使用
                self.dense[index] = entity.clone();
                *sparse = I::from_other(index as u32, entity);
                AllocAt::Using
            }
        }
    }

    /// 调用后必须再调用 flush_alloc_many 和 finish_alloc_many
    pub fn alloc_many(&mut self, count: usize) -> AllocManyState {
        self.verify_flushed();

        // 计算需要实际额外创建多少个全新的Entity
        let head = *self.head.get_mut();
        let free_count = self.dense.len() - head;
        let reused_count = free_count.min(count);
        let fresh_count = count.saturating_sub(reused_count);
        let fresh_start = self.dense.len();
        debug_assert!(((head + count) < u32::MAX as usize), "too many entities");
        let pending_start = head;
        let pending_end = pending_start + reused_count;
        let pending_range = pending_start..pending_end;
        for index in pending_range.clone() {
            self.dense[index].replace_flags(I::FlagType::default());
        }

        *self.head.get_mut() = head + count;

        AllocManyState {
            pending_range,
            fresh: fresh_start..(fresh_start + fresh_count),
        }
    }

    pub unsafe fn flush_alloc_many(&mut self, index: usize) -> I {
        let head = self.len();
        if self.dense.len() > index {
            let entity = &mut self.dense[index];
            let sparse_pos = Self::get_sparse_pos(entity.idx());
            self.sparse[sparse_pos.page].0[sparse_pos.slot] =
                I::new(index as u32, entity.version(), entity.flags());
            if index >= self.flushed_head {
                self.flushed_head = self.flushed_head + 1;
            }
            entity.clone()
        } else {
            if head > self.dense.len() {
                let entity = I::new(index as u32, 0, I::FlagType::default());
                let sparse_pos = Self::get_sparse_pos(entity.idx());
                if self.sparse.get(sparse_pos.page).is_none() {
                    self.sparse.push(Page::new());
                }
                self.sparse[sparse_pos.page].0[sparse_pos.slot] = entity.clone();
                self.dense.push(entity.clone());
                entity
            } else {
                panic!("entity id is out of range")
            }
        }
    }

    pub fn finish_alloc_many(&mut self, statte: &mut AllocManyState) {
        while let Some(index) = statte.next() {
            unsafe { self.flush_alloc_many(index) };
        }
    }

    pub unsafe fn resolve_unknown_version(&self, id: u32) -> I {
        let len = self.dense.len();

        if len > id as usize {
            let sparse_pos = Self::get_sparse_pos(id);
            let index = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
            self.dense[index.idx() as usize].clone()
        } else {
            panic!("entity id is out of range")
        }
    }

    fn swap_inner(&mut self, src: usize, moved: usize) {
        self.dense.swap(src, moved);

        let src_value = &self.dense[moved];
        let moved_value = &self.dense[src];
        let sparse_pos = Self::get_sparse_pos(src_value.idx());
        let moved_sparse_pos = Self::get_sparse_pos(moved_value.idx());

        self.sparse[moved_sparse_pos.page].0[moved_sparse_pos.slot] =
            I::from_other(src_value.idx(), moved_value);
        self.sparse[sparse_pos.page].0[sparse_pos.slot] =
            I::from_other(moved_value.idx(), src_value);
    }

    /// 删除实体，将其回收到 freelist。
    ///
    /// **版本号在此时立即递增**，保证后续 `reserve_entity` 拿到的是新版本。
    ///
    /// Return
    /// moved entity (end_entity)
    pub fn free(&mut self, id: I) -> Result<I, NoSuchId> {
        self.verify_flushed();

        let sparse_pos = Self::get_sparse_pos(id.idx());
        if self.sparse.get(sparse_pos.page).is_none() {
            return Err(NoSuchId);
        }
        if !self.sparse[sparse_pos.page].0[sparse_pos.slot].is_avalide() {
            return Err(NoSuchId);
        }

        let sparse_value = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
        if sparse_value.version() != id.version() {
            return Err(NoSuchId);
        }

        let head = *self.head.get_mut();
        let new_head = head - 1;
        let end_index = new_head; // 回收的目标位置
        let end_id = &self.dense[end_index];
        let moved = end_id.clone();
        let end_sparse_pos = Self::get_sparse_pos(end_id.idx());
        let end_sparse_value = &self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot];

        let index = sparse_value.idx() as usize;

        self.dense.swap(index, end_index);

        // 更新被 swap 移动的实体的 sparse 条目
        self.sparse[end_sparse_pos.page].0[end_sparse_pos.slot] =
            I::from_other(sparse_value.idx(), &end_sparse_value);

        // 标记被删除实体的 sparse 为 INVALID
        self.sparse[sparse_pos.page].0[sparse_pos.slot] = I::get_invalide_id();

        // ★ 关键：立即递增版本号存入 dense[end_index]，供后续 reserve_entity 使用
        let recycled = self.dense[end_index]
            .clone()
            .get_next_version(I::FlagType::default()); // 这里 flag 设为 default，通过 version 判断是否有效，避免 reserve 时需要 mut 改变Flag
        self.dense[end_index] = recycled;

        *self.head.get_mut() = new_head;
        self.flushed_head = new_head;
        Ok(moved)
    }

    /// 预留分配出 additional 个实体
    /// 这只会分配出需要的内存空间，而不会创建有效数据。这适用于需要避免频繁realloc的情况
    /// Return:
    ///     None => dense's capacity 足够用来分配 additional 个实体，因此没有实际内存分配
    ///     Some(shortfall: usize, page_shortfall Optional<usize>) => capacity 不够用来分配，紧密数组额外分配了 shortfall 个实体的内存
    ///         稀疏数组额外分配 page_shorfall 个单位内存(None 表示没有额外分配)
    pub fn reserve(&mut self, additional: usize) -> Option<(usize, Option<usize>)> {
        self.verify_flushed();

        let free_count = (self.dense.capacity() - self.len()) as isize;
        let shortfall = additional as isize - free_count;
        if shortfall > 0 {
            let shortfall = shortfall as usize;
            self.dense.reserve(shortfall);
            let end_sparse_pos = Self::get_sparse_pos((self.dense.capacity() + shortfall) as u32);
            let page_short_fall = end_sparse_pos.page - self.sparse.capacity();
            if page_short_fall > 0 {
                self.sparse.reserve(page_short_fall);
                for _ in 0..page_short_fall {
                    self.sparse.push(Page::new());
                }
                Some((shortfall, Some(page_short_fall)))
            } else {
                Some((shortfall, None))
            }
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.dense.clear();
        for page in &mut self.sparse {
            page.0.fill(I::get_invalide_id());
        }
    }

    /// 检查实体是否存在。
    ///
    /// 注意：已通过 `reserve_entity` 预留但未 flush 的实体也返回 `true`。
    #[inline(always)]
    pub fn has(&self, id: &I) -> bool {
        let idx = id.idx() as usize;
        let dense_len = self.dense.len();

        if idx < dense_len {
            // 可能在 dense 中：检查 sparse 条目
            let sparse_pos = Self::get_sparse_pos(id.idx());
            if self.sparse.get(sparse_pos.page).is_none() {
                return false;
            }
            let sparse_val = &self.sparse[sparse_pos.page].0[sparse_pos.slot];
            if sparse_val.version() == id.version() && sparse_val.is_avalide() {
                return true;
            }

            // 也可能在 reserved 区（sparse 尚未更新但 dense 中已分配）
            let head = self.head.load(Ordering::Relaxed);
            if idx < head && idx >= self.flushed_head {
                let dense_val = &self.dense[idx];
                return dense_val.idx() == id.idx() && dense_val.version() == id.version();
            }
            false
        } else {
            // 超出 dense：检查是否在 reserved 全新实体范围
            let head = self.head.load(Ordering::Relaxed);
            idx < head && id.version() == 0
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

    fn get_sparse_pos(idx: u32) -> SparsePos {
        let idx = idx as usize;
        let page = idx / SPARSE_PAGE_SIZE;
        let slot = idx % SPARSE_PAGE_SIZE;
        SparsePos::new(page, slot)
    }

    pub fn iter(&self) -> Iter<'_, I> {
        self.dense.iter()
    }

    pub fn dense(&self) -> &[I] {
        &self.dense
    }
}

impl<I> Index<usize> for SparseStroge<I>
where
    I: Id,
{
    type Output = I;

    fn index(&self, index: usize) -> &Self::Output {
        &self.dense[index]
    }
}

impl<I> Index<I> for SparseStroge<I>
where
    I: Id,
{
    type Output = I;

    fn index(&self, id: I) -> &Self::Output {
        let sparse_pos = Self::get_sparse_pos(id.idx());
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
            id.version() == index.version(),
            "The id's version is invalide! while get value, id: {:?}",
            id
        );
        &self.dense[index.idx() as usize]
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NoSuchId;
impl fmt::Display for NoSuchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("no such Id")
    }
}
impl error::Error for NoSuchId {}
