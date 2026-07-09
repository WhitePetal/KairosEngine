use std::{ffi::OsString, path::PathBuf};

use crate::kairos_editor::asset_registry::Guid;

// ============================================================
// ProjectNodeKind — 节点类型枚举
// ============================================================

/// 项目树节点的资源类型。
///
/// 通过文件扩展名映射：
///
/// | 扩展名        | 对应变体          |
/// |---------------|-------------------|
/// | (目录)        | `Directory`       |
/// | `.texture`    | `Texture`         |
/// | `.mesh`       | `Mesh`            |
/// | `.mat`        | `Material`        |
/// | `.audio`      | `Audio`           |
/// | `.wgsl`       | `Shader`          |
/// | `.asset`      | `GenericAsset`    |
/// | `.rs`         | `Script`          |
/// | `.md` / `.txt`| `Document`        |
/// | 其他          | `Unknown`         |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectNodeKind {
    Directory,
    Texture,
    Mesh,
    Material,
    Audio,
    Shader,
    GenericAsset,
    Script,
    Document,
    Toml,
    Unknown,
}

impl ProjectNodeKind {
    /// 从文件扩展名映射到节点类型。
    pub fn from_extension(ext: Option<&str>) -> Self {
        match ext {
            Some("texture") => Self::Texture,
            Some("mesh") => Self::Mesh,
            Some("mat") => Self::Material,
            Some("audio") => Self::Audio,
            Some("wgsl") => Self::Shader,
            Some("asset") => Self::GenericAsset,
            Some("rs") => Self::Script,
            Some("md" | "txt") => Self::Document,
            Some("toml") => Self::Toml,
            _ => Self::Unknown,
        }
    }

    /// 判断是否可展开（目录类型才有子节点）。
    pub fn is_expandable(&self) -> bool {
        matches!(self, Self::Directory)
    }

    pub fn suffix(&self) -> Option<&'static str> {
        match self {
            ProjectNodeKind::Directory => None,
            ProjectNodeKind::Texture => Some(".texture"),
            ProjectNodeKind::Mesh => Some(".mesh"),
            ProjectNodeKind::Material => Some(".mat"),
            ProjectNodeKind::Audio => Some(".audio"),
            ProjectNodeKind::Shader => Some(".wgsl"),
            ProjectNodeKind::GenericAsset => Some(".asset"),
            ProjectNodeKind::Script => Some(".rs"),
            ProjectNodeKind::Document => None,
            ProjectNodeKind::Toml => Some(".toml"),
            ProjectNodeKind::Unknown => None,
        }
    }

    /// 重命名时需要同步的关联扩展名（不含点）。
    /// Texture: png + texture + texture_bin；Mesh: mesh + mesh_bin；其余单文件。
    pub fn related_extensions(&self) -> &[&str] {
        match self {
            ProjectNodeKind::Texture => &["png", "texture", "texture_bin"],
            ProjectNodeKind::Mesh => &["mesh", "mesh_bin"],
            _ => &[],
        }
    }
}

// ============================================================
// ProjectTreeNode — 树节点数据
// ============================================================

/// 项目树中的一个节点，对应文件系统中的一个目录或资源文件。
///
/// 每个节点拥有一个全局唯一的 [`Guid`]，用于资产引用和依赖追踪。
/// 目录节点可展开，包含子节点（通过 petgraph 的边关系表达）。
#[derive(Debug, Clone)]
pub struct ProjectTreeNode {
    /// 全局唯一标识符（持久化于 [`AssetRegistry`]）
    pub guid: Guid,
    /// 文件名（不含路径前缀）
    pub name: OsString,
    /// 相对于项目根目录的完整路径（对于 Texture 类型，指向 .png 展示路径）
    pub path: PathBuf,
    /// 引擎资产路径（Texture: .texture 路径；其他: None 表示与 path 相同）
    pub asset_path: Option<PathBuf>,
    /// 节点类型
    pub kind: ProjectNodeKind,
}

impl ProjectTreeNode {
    pub fn new(guid: Guid, name: OsString, path: PathBuf, kind: ProjectNodeKind) -> Self {
        Self {
            guid,
            name,
            path,
            asset_path: None,
            kind,
        }
    }

    /// 创建带资产路径的节点（如 Texture：path=.png, asset_path=.texture）
    pub fn with_asset_path(
        guid: Guid,
        name: OsString,
        path: PathBuf,
        asset_path: PathBuf,
        kind: ProjectNodeKind,
    ) -> Self {
        Self {
            guid,
            name,
            path,
            asset_path: Some(asset_path),
            kind,
        }
    }

    pub fn name(&self) -> String {
        self.name.to_string_lossy().into_owned()
    }
}
