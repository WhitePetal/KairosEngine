pub const FLAG_MASK: u64 = 0xF;
pub const VERSION_MASK: u64 = 0xFFFFFFF;
pub const IDX_MASK: u64 = 0xFFFFFFFF;

pub const FLAG_MASK_LEN: u32 = FLAG_MASK.count_ones();
pub const VERSION_MASK_LEN: u32 = VERSION_MASK.count_ones();
pub const IDX_MASK_LEN: u32 = IDX_MASK.count_ones();

pub const IDX_MASK_OFFSET: u32 = IDX_MASK_LEN;
pub const VERSION_MASK_OFFSET: u32 = IDX_MASK_OFFSET + VERSION_MASK_LEN;
pub const FLAG_MASK_OFFSET: u32 = VERSION_MASK_OFFSET + FLAG_MASK_LEN;

pub const SPARSE_PAGE_SIZE: usize = 4096;

pub const WORLD_SCENE_CAPACITY: usize = 64;
pub const COMPONENT_TYPE_CAPACITY: usize = 1024;
