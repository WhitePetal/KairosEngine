use std::{ffi::OsString, path::PathBuf};

use crate::kairos_editor::asset_registry::{AssetKind, Guid};

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
    pub kind: AssetKind,
}

impl ProjectTreeNode {
    pub fn new(
        guid: Guid,
        name: OsString,
        path: PathBuf,
        asset_path: Option<PathBuf>,
        kind: AssetKind,
    ) -> Self {
        Self {
            guid,
            name,
            path,
            asset_path,
            kind,
        }
    }

    pub fn name(&self) -> String {
        self.name.to_string_lossy().into_owned()
    }
}
