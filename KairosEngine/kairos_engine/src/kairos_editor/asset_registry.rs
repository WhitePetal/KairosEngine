use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};
use uuid::Uuid;

use crate::asset::io::get_meta_path;

/// 项目资源类型。
///
/// 通过文件扩展名映射：
///
/// | 扩展名        | 对应变体          |
/// |---------------|-------------------|
/// | (Directory)   | `Directory`       |
/// | `.png`        | `Texture`         |
/// | `.glb`        | `Mesh`            |
/// | `.mat`        | `Material`        |
/// | `.audio`      | `Audio`           |
/// | `.wgsl`       | `Shader`          |
/// | `.rs`         | `Script`          |
/// | `.md`         | `Document`        |
/// | `.toml`       | `Toml`            |
/// | `.ttf`        | `Font`            |
/// | Other         | `Unknown`         |
///
/// 图形资产（`Texture` / `Mesh`）的节点指向源文件（`.png` / `.glb`）；
/// 其 `.meta` 边车与 `imported_assets/Default` 下的成品都是伴生文件，
/// 不出现在树中（见 ADR 0002 / 0004）。
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

    /// 资产文件的磁盘扩展名。
    ///
    /// 图形资产在 `.meta` 迁移后以源文件本身为项目资产：`Texture` 是 `.png`，
    /// `Mesh` 是 `.glb`；引擎实际加载的是 processor 写在
    /// `imported_assets/Default` 下的成品（ADR 0002 / 0004）。
    pub const fn extension(&self) -> Option<&'static str> {
        match self {
            AssetKind::Directory | AssetKind::Unknown => None,
            AssetKind::Texture => Some("png"),
            AssetKind::Mesh => Some("glb"),
            AssetKind::Material => Some("mat"),
            AssetKind::Audio => Some("audio"),
            AssetKind::Shader => Some("wgsl"),
            AssetKind::Script => Some("rs"),
            AssetKind::Document => Some("md"),
            AssetKind::Toml => Some("toml"),
            AssetKind::Font => Some("ttf"),
        }
    }

    /// 该类型是否由 asset processor 加工。
    ///
    /// 加工类资产的项目节点指向源文件（`.png` / `.glb`），源文件旁有一份
    /// `.meta` 边车（`AssetAction::Process`）；引擎通过成品目录里的
    /// `AssetAction::Load` 边车加载（ADR 0002 / 0004）。
    pub const fn is_processed(&self) -> bool {
        matches!(self, Self::Texture | Self::Mesh)
    }

    /// 伴生扩展名：仅作为主资产的附属文件，不应在项目树中单独显示。
    ///
    /// `.meta` 是每个资产旁统一的边车（ADR 0002）；旧 `_bin` 成品扩展名已退役，
    /// 它们会作为 `Unknown` 一并隐藏。
    pub fn is_companion_extension(ext: &str) -> bool {
        ext == "meta"
    }

    /// 展示用的扩展名（带前导点），如 `.png`。
    pub fn suffix(&self) -> Option<String> {
        self.extension().map(|ext| format!(".{ext}"))
    }

    /// 重命名/删除时需要同步的兄弟文件后缀（含点，从节点文件名 stem 起算）。
    ///
    /// 主资产 + 它的 `.meta` 边车，例如 `foo.png` → `[".png", ".png.meta"]`。
    /// 目录与 `Unknown` 无后缀。
    pub fn related_suffixes(&self) -> Vec<String> {
        match self.extension() {
            Some(ext) => vec![format!(".{ext}"), format!(".{ext}.meta")],
            None => Vec::new(),
        }
    }
}

// ============================================================
// AssetRoots — 扫描时的源根 / 成品根
// ============================================================

/// 扫描资产树时的一组根：源根，以及相对于它的成品根（默认
/// `imported_assets/Default`，见 ADR 0004）。
#[derive(Debug, Clone)]
pub struct AssetRoots {
    /// 源根。编辑器里为空，表示进程工作目录。
    pub source: PathBuf,
    /// 成品根，相对 [`source`](Self::source)。
    pub processed: PathBuf,
}

impl AssetRoots {
    /// 以 `source` 为源根、默认成品根构造。
    pub fn new(source: PathBuf) -> Self {
        Self {
            source,
            processed: PathBuf::from(crate::asset::AssetOptions::DEFAULT_PROCESSED_FILE_PATH),
        }
    }
}

/// 成品路径：`source_root / processed_root /` + 源相对路径（镜像源目录布局）。
pub fn processed_asset_path(source: &Path, roots: &AssetRoots) -> PathBuf {
    let relative = if roots.source.as_os_str().is_empty() {
        source.to_path_buf()
    } else {
        source
            .strip_prefix(&roots.source)
            .unwrap_or(source)
            .to_path_buf()
    };
    roots.source.join(&roots.processed).join(relative)
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

    /// 分析路径：根据扩展名识别 [`AssetKind`] 并返回对应 GUID 与引擎资产路径。
    ///
    /// - 伴生文件（`.meta` 边车）：返回 `None`，应被隐藏。
    /// - 加工类源文件（`.png` / `.glb`）：其 `.meta` 边车存在时识别为资产，
    ///   引擎资产路径配对到 `imported_assets/Default` 下镜像源相对路径的成品；
    ///   缺少 `.meta`（尚未导入）时跳过。
    /// - 普通主资产文件（`.mat`, `.wgsl`, ...）：直接识别，引擎资产即自身。
    pub fn analyse_path(
        &mut self,
        path: &Path,
        roots: &AssetRoots,
    ) -> Option<(AssetKind, Guid, Option<PathBuf>)> {
        let ext = path.extension().and_then(|e| e.to_str())?;

        // 伴生文件：隐藏
        if AssetKind::is_companion_extension(ext) {
            return None;
        }

        let kind = AssetKind::from_extension(Some(ext));
        if kind == AssetKind::Unknown {
            return None;
        }

        // 加工类资产以源文件为节点；未导入（无 `.meta` 边车）时跳过，同时把
        // 引擎资产路径配对到同名的成品。
        let asset_path = if kind.is_processed() {
            if !get_meta_path(path).exists() {
                return None;
            }
            Some(processed_asset_path(path, roots))
        } else {
            None
        };

        let guid = self.get_or_create_guid(path);
        Some((kind, guid, asset_path))
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
