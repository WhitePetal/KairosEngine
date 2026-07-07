pub mod texture_path;
pub mod tree_node;

use std::{ffi::OsString, path::Path};

use petgraph::{
    Directed, Graph,
    graph::{Edges, NodeIndex},
    visit::NodeIndexable,
};

use crate::kairos_editor::asset_registry::AssetRegistry;
use tree_node::{ProjectNodeKind, ProjectTreeNode};

// ============================================================
// ProjectPathGraph — 项目目录树（基于 petgraph）
// ============================================================

/// 以 petgraph 有向图构建的项目目录层级结构。
///
/// 节点权重为 [`ProjectTreeNode`]，边权重为 `()`。
/// 每个节点通过 [`AssetRegistry`] 持有持久化的 GUID。
pub struct ProjectPathGraph {
    graph: Graph<ProjectTreeNode, ()>,
}

impl ProjectPathGraph {
    /// 扫描项目根目录，构建完整目录树。
    ///
    /// `registry` 用于分配/查找已有 GUID，保证跨会话 GUID 稳定。
    pub fn new(registry: &mut AssetRegistry) -> Self {
        let mut graph = Graph::new();
        let root_path = Path::new("./");

        // 给根目录 itself 注册 GUID
        let root_guid = registry.get_or_create_guid(root_path);
        let root_name = root_path
            .canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_os_string()))
            .unwrap_or_else(|| OsString::from("KairosEngine"));
        let root_node_data = ProjectTreeNode::new(
            root_guid,
            root_name,
            root_path.to_path_buf(),
            ProjectNodeKind::Directory,
        );
        let root_node = graph.add_node(root_node_data);

        // 递归扫描
        Self::scan_dir(root_path, root_node, &mut graph, registry);

        Self { graph }
    }

    /// 刷新整个树（重新扫描）。
    ///
    /// 会保留 Registry 中已有的 GUID，新文件自动注册。
    pub fn refresh(&mut self, registry: &mut AssetRegistry) {
        self.graph.clear();
        let root_path = Path::new("./");

        let root_guid = registry.get_or_create_guid(root_path);
        let root_name = root_path
            .canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_os_string()))
            .unwrap_or_else(|| OsString::from("KairosEngine"));
        let root_node_data = ProjectTreeNode::new(
            root_guid,
            root_name,
            root_path.to_path_buf(),
            ProjectNodeKind::Directory,
        );
        let root_node = self.graph.add_node(root_node_data);

        Self::scan_dir(root_path, root_node, &mut self.graph, registry);
    }

    // ----------------------------------------------------------
    // 递归扫描
    // ----------------------------------------------------------

    fn scan_dir(
        dir_path: &Path,
        parent_node: NodeIndex,
        graph: &mut Graph<ProjectTreeNode, ()>,
        registry: &mut AssetRegistry,
    ) {
        let Ok(read_dir) = std::fs::read_dir(dir_path) else {
            return;
        };

        // 先收集目录，再收集文件（保证目录在前）
        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in read_dir {
            let Ok(entry) = entry else { continue };
            let path = entry.path();

            // 跳过隐藏文件/目录和 target 目录
            if Self::should_skip(&path) {
                continue;
            }

            if path.is_dir() {
                dirs.push(path);
            } else {
                files.push(path);
            }
        }

        // 先处理目录
        for path in dirs {
            let guid = registry.get_or_create_guid(&path);
            let name = path
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_default();
            let node_data =
                ProjectTreeNode::new(guid, name, path.clone(), ProjectNodeKind::Directory);
            let child_node = graph.add_node(node_data);
            graph.add_edge(parent_node, child_node, ());
            Self::scan_dir(&path, child_node, graph, registry);
        }

        // 再处理文件
        for path in files {
            let ext = path.extension().and_then(|e| e.to_str());
            let kind = ProjectNodeKind::from_extension(ext);

            // 跳过无法识别的文件类型（.glb, .png, .ogg 等源文件不纳入资源树）
            if kind == ProjectNodeKind::Unknown {
                continue;
            }

            let guid = registry.get_or_create_guid(&path);
            let name = path
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_default();
            let node_data = ProjectTreeNode::new(guid, name, path.clone(), kind);
            let child_node = graph.add_node(node_data);
            graph.add_edge(parent_node, child_node, ());
        }
    }

    /// 判断路径是否应跳过扫描。
    fn should_skip(path: &Path) -> bool {
        // 跳过隐藏文件/目录
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                return true;
            }
        }
        // 跳过 target 构建目录
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name == "target" {
                return true;
            }
        }
        false
    }

    // ----------------------------------------------------------
    // 查询 API
    // ----------------------------------------------------------

    /// 获取根节点索引（始终为 `0`）。
    pub fn get_root_node(&self) -> NodeIndex {
        self.graph.from_index(0)
    }

    /// 根据节点索引获取节点数据。
    pub fn get_node(&self, node: NodeIndex) -> Option<&ProjectTreeNode> {
        self.graph.node_weight(node)
    }

    /// 获取节点的所有出边（即子节点列表）。
    pub fn get_edges(&self, node: NodeIndex) -> Edges<'_, (), Directed> {
        self.graph.edges(node)
    }

    /// 返回图中节点总数。
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }
}

// ============================================================
// 向后兼容 — 保留旧的 ProjectPath / TexturePath 访问方式
// ============================================================

/// 旧版 `ProjectPath` 枚举的替代访问 — 通过 `ProjectTreeNode` + `ProjectNodeKind`
/// 提供等价信息。此模块在后续 UI 重写后将移除。
pub mod compat {
    use super::{
        ProjectPathGraph,
        tree_node::{ProjectNodeKind, ProjectTreeNode},
    };
    use crate::kairos_editor::project_path_tree::texture_path::TexturePath;

    /// 从 `ProjectTreeNode` 构建 `TexturePath`（向后兼容）。
    pub fn to_texture_path(node: &ProjectTreeNode) -> Option<TexturePath> {
        if node.kind != ProjectNodeKind::Texture {
            return None;
        }
        Some(TexturePath::new(
            node.path.clone(),
            node.path.clone(),
            node.name.clone(),
        ))
    }

    /// 便捷方法：通过图 + 节点索引获取 `TexturePath`。
    pub fn get_texture_path(
        graph: &ProjectPathGraph,
        node: petgraph::graph::NodeIndex,
    ) -> Option<TexturePath> {
        graph.get_node(node).and_then(to_texture_path)
    }

    /// 判断节点是否为目录。
    pub fn is_dir(node: &ProjectTreeNode) -> bool {
        node.kind == ProjectNodeKind::Directory
    }

    /// 判断节点是否为通用资产。
    pub fn is_asset(node: &ProjectTreeNode) -> bool {
        node.kind == ProjectNodeKind::GenericAsset
    }
}
