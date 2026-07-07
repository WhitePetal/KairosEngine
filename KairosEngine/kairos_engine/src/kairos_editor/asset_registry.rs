use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================
// Guid
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Guid(Uuid);

impl Guid {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================
// AssetEntry — 用于序列化的单条记录
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AssetEntry {
    guid: Guid,
    path: PathBuf,
}

// ============================================================
// AssetRegistry — guid ↔ path 双向映射表
// ============================================================

/// 持久化的资产注册表，维护所有项目资源（含目录）的 GUID ↔ Path 双向映射。
///
/// 磁盘存储路径: `Preferences/asset_registry.toml`
pub struct AssetRegistry {
    guid_to_path: HashMap<Guid, PathBuf>,
    path_to_guid: HashMap<PathBuf, Guid>,
}

impl AssetRegistry {
    const REGISTRY_PATH: &'static str = "Library/asset_registry.toml";

    // ----------------------------------------------------------
    // 构造 / 持久化
    // ----------------------------------------------------------

    pub fn new() -> Self {
        Self {
            guid_to_path: HashMap::new(),
            path_to_guid: HashMap::new(),
        }
    }

    /// 从磁盘加载已有注册表，如果文件不存在则返回空表。
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Path::new(Self::REGISTRY_PATH);
        if !path.exists() {
            return Ok(Self::new());
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            format!(
                "AssetRegistry: failed to read '{}': {}",
                Self::REGISTRY_PATH,
                e
            )
        })?;

        let entries: Vec<AssetEntry> = toml::from_str(&content).map_err(|e| {
            format!(
                "AssetRegistry: failed to parse '{}': {}",
                Self::REGISTRY_PATH,
                e
            )
        })?;

        let mut registry = Self::new();
        for entry in entries {
            registry.insert_entry(entry);
        }
        Ok(registry)
    }

    /// 将注册表持久化到磁盘。
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let entries: Vec<AssetEntry> = self
            .guid_to_path
            .iter()
            .map(|(&guid, path)| AssetEntry {
                guid,
                path: path.clone(),
            })
            .collect();

        let content = toml::to_string(&entries)
            .map_err(|e| format!("AssetRegistry: failed to serialize: {}", e))?;

        // 确保目录存在
        if let Some(parent) = Path::new(Self::REGISTRY_PATH).parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(Self::REGISTRY_PATH, content).map_err(|e| {
            format!(
                "AssetRegistry: failed to write '{}': {}",
                Self::REGISTRY_PATH,
                e
            )
        })?;

        Ok(())
    }

    // ----------------------------------------------------------
    // 查询
    // ----------------------------------------------------------

    /// 根据 GUID 查询路径。
    pub fn get_path(&self, guid: &Guid) -> Option<&PathBuf> {
        self.guid_to_path.get(guid)
    }

    /// 根据路径查询 GUID。
    pub fn get_guid(&self, path: &Path) -> Option<&Guid> {
        self.path_to_guid.get(path)
    }

    /// 检查路径是否已被注册。
    pub fn contains_path(&self, path: &Path) -> bool {
        self.path_to_guid.contains_key(path)
    }

    // ----------------------------------------------------------
    // 注册 / 更新 / 移除
    // ----------------------------------------------------------

    /// 为路径获取或创建 GUID：已有则返回已有 GUID，否则生成新 GUID 并注册。
    pub fn get_or_create_guid(&mut self, path: &Path) -> Guid {
        if let Some(guid) = self.path_to_guid.get(path) {
            return *guid;
        }
        let guid = Guid::new();
        self.guid_to_path.insert(guid, path.to_path_buf());
        self.path_to_guid.insert(path.to_path_buf(), guid);
        guid
    }

    /// 手动注册一个路径（使用已有 GUID），如果路径已存在则更新。
    pub fn register(&mut self, guid: Guid, path: PathBuf) {
        // 如果该 guid 之前指向旧路径，先清理反向映射
        if let Some(old_path) = self.guid_to_path.remove(&guid) {
            self.path_to_guid.remove(&old_path);
        }
        // 如果该路径之前指向旧 guid，先清理正向映射
        if let Some(old_guid) = self.path_to_guid.remove(&path) {
            self.guid_to_path.remove(&old_guid);
        }
        self.guid_to_path.insert(guid, path.clone());
        self.path_to_guid.insert(path, guid);
    }

    /// 移除路径对应的注册记录。
    pub fn unregister(&mut self, path: &Path) {
        if let Some(guid) = self.path_to_guid.remove(path) {
            self.guid_to_path.remove(&guid);
        }
    }

    /// 更新路径（保持 GUID 不变）。用于资源移动/重命名场景。
    pub fn update_path(&mut self, old_path: &Path, new_path: &Path) {
        if let Some(guid) = self.path_to_guid.remove(old_path) {
            self.guid_to_path.insert(guid, new_path.to_path_buf());
            self.path_to_guid.insert(new_path.to_path_buf(), guid);
        }
    }

    // ----------------------------------------------------------
    // 内部 helpers
    // ----------------------------------------------------------

    fn insert_entry(&mut self, entry: AssetEntry) {
        self.guid_to_path.insert(entry.guid, entry.path.clone());
        self.path_to_guid.insert(entry.path, entry.guid);
    }
}
