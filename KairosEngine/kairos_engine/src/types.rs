use std::{
    any::TypeId,
    collections::HashMap,
    hash::{BuildHasher, BuildHasherDefault, Hasher},
};

///
/// TypeId 本身就是一个哈希值，因此作为哈希表的Key时不需要被再次哈希
/// 该Hasher就是让TypeId本身值作为TypeId的哈希值，以优化哈希表访问效率
#[derive(Debug, Default)]
pub struct TypeIdHasher {
    hash: u64,
}

impl Hasher for TypeIdHasher {
    fn write_u64(&mut self, i: u64) {
        // 每个类型只能被Hash一次，即此时self.hash应该为0
        debug_assert_eq!(self.hash, 0);

        self.hash = i;
    }
    fn write_u128(&mut self, i: u128) {
        debug_assert_eq!(self.hash, 0);

        // u64位数足够，直接downcast到u64
        self.hash = i as u64;
    }
    fn write(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.hash, 0);

        // 只有在 TypeId 既不是 u64, 也不是 u128 时才会发生，这通常不会出现
        let mut hasher = foldhash::fast::FixedState::with_seed(0xb334867b740a29a5).build_hasher();
        hasher.write(bytes);
        self.hash = hasher.finish();
    }
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type TypeIdMap<V> = HashMap<TypeId, V, BuildHasherDefault<TypeIdHasher>>;

#[derive(Debug)]
pub struct OrderedTypeIdMap<V>(Box<[(TypeId, V)]>);
impl<V> OrderedTypeIdMap<V> {
    pub fn new(iter: impl Iterator<Item = (TypeId, V)>) -> Self {
        let mut vals = iter.collect::<Box<[_]>>();
        vals.sort_unstable_by_key(|(id, _)| *id);
        Self(vals)
    }

    pub fn search(&self, id: &TypeId) -> Option<usize> {
        self.0.binary_search_by_key(id, |(id, _)| *id).ok()
    }

    pub fn contains_key(&self, id: &TypeId) -> bool {
        self.search(id).is_some()
    }

    pub fn get(&self, id: &TypeId) -> Option<&V> {
        self.search(id).map(move |idx| &self.0[idx].1)
    }
}
