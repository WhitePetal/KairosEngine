use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use uuid::Uuid;

use crate::inputs::Input::S;

/// 项目资源类型。
///
/// 通过文件扩展名映射：
///
/// | 扩展名        | 对应变体          |
/// |---------------|-------------------|
/// | (Directory)   | `Directory`       |
/// | `.texture`    | `Texture`         |
/// | `.mesh`       | `Mesh`            |
/// | `.mat`        | `Material`        |
/// | `.audio`      | `Audio`           |
/// | `.wgsl`       | `Shader`          |
/// | `.asset`      | `GenericAsset`    |
/// | `.rs`         | `Script`          |
/// | `.md` / `.txt`| `Document`        |
/// | `.ttf`        | `Font`            |
/// | Other         | `Unknown`         |
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum AssetKind {
    Directory,
    Texture,
    Mesh,
    Material,
    Audio,
    Shader,
    Script,
    Document,
    Toml,
    Font,
    Unknown,
}

impl AssetKind {
    /// 从文件扩展名映射到节点类型（仅匹配主资产扩展名）。
    pub fn from_extension(ext: Option<&str>) -> Self {
        let Some(ext) = ext else {
            return Self::Unknown;
        };
        for kind in Self::iter() {
            if kind.extension() == Some(ext) {
                return kind;
            }
        }
        Self::Unknown
    }

    /// 判断是否可展开（目录类型才有子节点）。
    pub fn is_expandable(&self) -> bool {
        matches!(self, Self::Directory)
    }

    /// 扩展名，用于创建新文件。
    /// 必须为 `const fn` 内联匹配以保持 `'static` 生命周期。
    pub const fn extension(&self) -> Option<&'static str> {
        match self {
            AssetKind::Directory | AssetKind::Unknown => None,
            AssetKind::Texture => Some("texture"),
            AssetKind::Mesh => Some("mesh"),
            AssetKind::Material => Some("mat"),
            AssetKind::Audio => Some("audio"),
            AssetKind::Shader => Some("wgsl"),
            AssetKind::Script => Some("rs"),
            AssetKind::Document => Some("md"),
            AssetKind::Toml => Some("toml"),
            AssetKind::Font => Some("ttf"),
        }
    }

    /// 源文件扩展名：导入前的原始文件，需要在同目录存在对应的主资产文件才能识别。
    /// 例如 `.png` → 检查 `.texture` 是否存在。
    pub const fn source_extensions(&self) -> Option<&'static str> {
        match self {
            AssetKind::Texture => Some("png"),
            AssetKind::Mesh => Some("glb"),
            AssetKind::Directory => None,
            AssetKind::Material => None,
            AssetKind::Audio => None,
            AssetKind::Shader => None,
            AssetKind::Script => None,
            AssetKind::Document => None,
            AssetKind::Toml => None,
            AssetKind::Font => None,
            AssetKind::Unknown => None,
        }
    }

    /// 根据源文件扩展名查找对应的 AssetKind。遍历所有变体的 [`source_extensions`]。
    pub fn from_source_extension(ext: &str) -> Option<Self> {
        for kind in Self::iter() {
            if kind.source_extensions() == Some(ext) {
                return Some(kind);
            }
        }
        None
    }

    /// 伴生扩展名：仅作为主资产的附属文件，不应在项目树中单独显示。
    pub const fn companion_extensions(&self) -> Option<&'static str> {
        match self {
            AssetKind::Texture => Some("texture_bin"),
            AssetKind::Mesh => Some("mesh_bin"),
            AssetKind::Directory => None,
            AssetKind::Material => None,
            AssetKind::Audio => None,
            AssetKind::Shader => None,
            AssetKind::Script => None,
            AssetKind::Document => None,
            AssetKind::Toml => None,
            AssetKind::Font => None,
            AssetKind::Unknown => None,
        }
    }

    /// 判断扩展名是否为伴生文件（应隐藏）。遍历所有变体的 [`companion_extensions`]。
    pub fn is_companion_extension(ext: &str) -> bool {
        for kind in Self::iter() {
            if kind.companion_extensions() == Some(ext) {
                return true;
            }
        }
        false
    }

    /// 判断扩展名是否属于有源文件映射的主资产扩展名（如 `texture`, `mesh`）。
    /// 这类主资产文件在扫描时由源文件（png/glb）负责创建节点，自身应跳过。
    pub fn is_imported_primary_extension(ext: &str) -> bool {
        for kind in Self::iter() {
            if kind.source_extensions().is_some() && kind.extension() == Some(ext) {
                return true;
            }
        }
        false
    }

    pub fn suffix(&self) -> Option<String> {
        let extension = self.extension();
        match extension {
            Some(ext) => Some(format!(".{}", ext)),
            None => None,
        }
    }

    /// 重命名/删除时需要同步的所有关联扩展名（不含点）。
    /// 包含源文件、主资产和伴生文件。
    pub fn related_extensions(&self) -> Vec<&str> {
        let mut all: Vec<&str> = Vec::new();
        if let Some(ext) = self.source_extensions() {
            all.push(ext);
        }
        if let Some(ext) = self.extension() {
            all.push(ext);
        }
        if let Some(ext) = self.companion_extensions() {
            all.push(ext);
        }
        all
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializedAssetEntries {
    entries: Vec<AssetEntry>,
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

        let entries: SerializedAssetEntries = toml::from_str(&content).map_err(|e| {
            format!(
                "AssetRegistry: failed to parse '{}': {}",
                Self::REGISTRY_PATH,
                e
            )
        })?;

        let mut registry = Self::new();
        for entry in entries.entries {
            registry.insert_entry(entry);
        }
        Ok(registry)
    }

    /// 将注册表持久化到磁盘。
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut entries: Vec<AssetEntry> = self
            .guid_to_path
            .iter()
            .map(|(&guid, path)| AssetEntry {
                guid,
                path: path.clone(),
            })
            .collect();

        // 按路径排序，保证跨运行输出稳定（HashMap 迭代顺序不确定）
        entries.sort_by(|a, b| a.path.cmp(&b.path));

        let entries = SerializedAssetEntries { entries };

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

    /// 分析路径：根据扩展名识别 [`AssetKind`] 并返回对应 GUID。
    ///
    /// - 源文件（如 `.png`）：检查同目录是否存在主资产文件（如 `.texture`），
    ///   若存在则返回主资产路径的 GUID + Kind + AssetPath。
    /// - 主资产文件（如 `.mat`, `.wgsl`）：直接识别并返回。
    /// - 伴生文件（如 `.texture_bin`）：返回 `None`，应被隐藏。
    pub fn analyse_path(&mut self, path: &PathBuf) -> Option<(AssetKind, Guid, Option<PathBuf>)> {
        let ext = path.extension().and_then(|e| e.to_str())?;

        // 伴生文件：隐藏
        if AssetKind::is_companion_extension(ext) {
            return None;
        }

        // 源文件：检查主资产伴生文件
        if let Some(kind) = AssetKind::from_source_extension(ext) {
            if let Some(primary_ext) = kind.extension() {
                let asset_path = path.with_extension(primary_ext);
                if asset_path.exists() {
                    let guid = self.get_or_create_guid(&asset_path);
                    return Some((kind, guid, Some(asset_path)));
                }
            }
            return None; // 源文件存在但未导入 → 跳过
        }

        // 有源文件的资产的主扩展名（如 `.texture`, `.mesh`）：由源文件处理，跳过
        if AssetKind::is_imported_primary_extension(ext) {
            return None;
        }

        // 普通主资产文件
        let kind = AssetKind::from_extension(Some(ext));
        if kind == AssetKind::Unknown {
            return None;
        }

        let guid = self.get_or_create_guid(path);
        Some((kind, guid, None))
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
