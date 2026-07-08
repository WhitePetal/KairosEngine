pub mod create_request;
pub mod texture_path;
pub mod tree_node;

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use petgraph::{
    Directed, Graph,
    graph::{Edges, NodeIndex},
    visit::{EdgeRef, NodeIndexable},
};

use crate::kairos_editor::asset_registry::AssetRegistry;
use create_request::CreateRequest;
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
        Self::new_at_root(Path::new("./"), registry)
    }

    /// 同 [`new`]，但以自定义路径作为根目录（用于测试）。
    pub fn new_at_root(root_path: &Path, registry: &mut AssetRegistry) -> Self {
        let mut graph = Graph::new();

        let root_guid = registry.get_or_create_guid(root_path);
        let root_name = root_path
            .canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_os_string()))
            .unwrap_or_else(|| OsString::from("root"));
        let root_node_data = ProjectTreeNode::new(
            root_guid,
            root_name,
            root_path.to_path_buf(),
            ProjectNodeKind::Directory,
        );
        let root_node = graph.add_node(root_node_data);

        Self::scan_dir(root_path, root_node, &mut graph, registry);

        Self { graph }
    }

    /// 刷新整个树（重新扫描）。
    ///
    /// 会保留 Registry 中已有的 GUID，新文件自动注册。
    pub fn refresh(&mut self, registry: &mut AssetRegistry) {
        let root_path = self
            .graph
            .node_weight(self.get_root_node())
            .map(|n| n.path.clone())
            .unwrap_or_else(|| PathBuf::from("./"));

        self.graph.clear();
        let root_guid = registry.get_or_create_guid(&root_path);
        let root_name = OsString::from("KairosEngine");
        let root_node_data = ProjectTreeNode::new(
            root_guid,
            root_name,
            root_path.clone(),
            ProjectNodeKind::Directory,
        );
        let root_node = self.graph.add_node(root_node_data);

        Self::scan_dir(&root_path, root_node, &mut self.graph, registry);
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

            match ext {
                Some("png") => {
                    let texture_path = path.with_extension("texture");
                    if texture_path.exists() {
                        let guid = registry.get_or_create_guid(&texture_path);
                        let name = path
                            .file_name()
                            .map(|n| n.to_os_string())
                            .unwrap_or_default();
                        let node_data = ProjectTreeNode::with_asset_path(
                            guid,
                            name,
                            path.clone(),
                            texture_path,
                            ProjectNodeKind::Texture,
                        );
                        let child_node = graph.add_node(node_data);
                        graph.add_edge(parent_node, child_node, ());
                    }
                    continue;
                }
                Some("glb") => {
                    let mesh_path = path.with_extension("mesh");
                    if mesh_path.exists() {
                        let guid = registry.get_or_create_guid(&mesh_path);
                        let name = path
                            .file_name()
                            .map(|n| n.to_os_string())
                            .unwrap_or_default();
                        let node_data = ProjectTreeNode::with_asset_path(
                            guid,
                            name,
                            path.clone(),
                            mesh_path,
                            ProjectNodeKind::Mesh,
                        );
                        let child_node = graph.add_node(node_data);
                        graph.add_edge(parent_node, child_node, ());
                    }
                    continue;
                }
                Some("texture" | "texture_bin" | "mesh" | "mesh_bin") => {
                    continue;
                }
                _ => {}
            }

            let kind = ProjectNodeKind::from_extension(ext);

            // 跳过无法识别的文件类型
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
        // 跳过 target 构建目录和 Library 引擎内部目录
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            match name {
                "target" | "Library" => {
                    return true;
                }
                _ => {}
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

    /// 获取节点的父节点索引（文件用于回退到所在目录）。
    /// 文件被选中时 content_panel 应展示其所在目录的子节点。
    pub fn get_parent(&self, node: NodeIndex) -> Option<NodeIndex> {
        use petgraph::Direction;
        self.graph
            .neighbors_directed(node, Direction::Incoming)
            .next()
    }

    // ----------------------------------------------------------
    // 写操作
    // ----------------------------------------------------------

    /// 在指定父目录下创建新节点（目录或文件）。
    ///
    /// 校验：父节点存在且为目录、无同名兄弟节点。
    /// 成功后会在文件系统中创建对应实体并更新图与 registry。
    pub fn create_node(
        &mut self,
        registry: &mut AssetRegistry,
        request: CreateRequest,
    ) -> Result<NodeIndex, String> {
        // ---- 1. 校验父节点 ----
        let parent_data = self
            .graph
            .node_weight(request.parent_node)
            .ok_or_else(|| "parent node not found in graph".to_string())?;

        if parent_data.kind != ProjectNodeKind::Directory {
            return Err("parent node is not a directory".to_string());
        }

        // ---- 2. 校验无同名兄弟 ----
        let name_exists = self.graph.edges(request.parent_node).any(|e| {
            self.graph
                .node_weight(e.target())
                .is_some_and(|n| n.name.to_string_lossy().eq_ignore_ascii_case(&request.name))
        });
        if name_exists {
            return Err(format!(
                "'{}' already exists in '{}'",
                request.name,
                parent_data.name.to_string_lossy()
            ));
        }

        // ---- 3. 构建路径 + 文件系统操作 ----
        let full_path = parent_data.path.join(&request.name);

        match request.kind {
            ProjectNodeKind::Directory => {
                std::fs::create_dir(&full_path).map_err(|e| {
                    format!("failed to create directory '{}': {e}", full_path.display())
                })?;
            }
            _ => {
                return Err(format!(
                    "file creation ({:?}) is not yet implemented",
                    request.kind
                ));
            }
        }

        // ---- 4. 注册 GUID ----
        let guid = registry.get_or_create_guid(&full_path);
        let name = request.name.into();
        let kind = request.kind;

        // ---- 5. 更新图 ----
        let node_data = ProjectTreeNode::new(guid, name, full_path, kind);
        let new_node = self.graph.add_node(node_data);
        self.graph.add_edge(request.parent_node, new_node, ());

        Ok(new_node)
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

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    /// 在临时目录下构造一个以该目录为根的 ProjectPathGraph。
    fn setup() -> (TempDir, ProjectPathGraph, AssetRegistry) {
        let tmp = TempDir::new().unwrap();
        let mut registry = AssetRegistry::new();
        let graph = ProjectPathGraph::new_at_root(tmp.path(), &mut registry);
        (tmp, graph, registry)
    }

    // ---- create_node ----

    #[test]
    fn create_directory_success() {
        let (tmp, mut graph, mut registry) = setup();
        let root = graph.get_root_node();

        let result = graph.create_node(
            &mut registry,
            CreateRequest {
                parent_node: root,
                name: "new_dir".into(),
                kind: ProjectNodeKind::Directory,
            },
        );

        assert!(result.is_ok(), "{result:?}");
        let node = result.unwrap();
        let data = graph.get_node(node).unwrap();
        assert_eq!(data.kind, ProjectNodeKind::Directory);
        assert!(tmp.path().join("new_dir").exists());
        assert!(registry.get_guid(&data.path).is_some());
    }

    #[test]
    fn create_duplicate_name_fails() {
        let (_tmp, mut graph, mut registry) = setup();
        let root = graph.get_root_node();

        let req = CreateRequest {
            parent_node: root,
            name: "dup".into(),
            kind: ProjectNodeKind::Directory,
        };
        graph.create_node(&mut registry, req.clone()).unwrap();
        let result = graph.create_node(&mut registry, req);
        assert!(result.is_err());
    }

    #[test]
    fn create_file_not_implemented() {
        let (_tmp, mut graph, mut registry) = setup();
        let root = graph.get_root_node();

        let result = graph.create_node(
            &mut registry,
            CreateRequest {
                parent_node: root,
                name: "test.txt".into(),
                kind: ProjectNodeKind::Document,
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet implemented"));
    }

    #[test]
    fn create_invalid_parent_fails() {
        let (_tmp, mut graph, mut registry) = setup();
        let bogus = NodeIndex::new(99999);

        let result = graph.create_node(
            &mut registry,
            CreateRequest {
                parent_node: bogus,
                name: "orphan".into(),
                kind: ProjectNodeKind::Directory,
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn parent_must_be_directory() {
        let (_tmp, mut graph, mut registry) = setup();
        let root = graph.get_root_node();

        let dir = graph
            .create_node(
                &mut registry,
                CreateRequest {
                    parent_node: root,
                    name: "subdir".into(),
                    kind: ProjectNodeKind::Directory,
                },
            )
            .unwrap();

        assert_eq!(
            graph.get_node(dir).unwrap().kind,
            ProjectNodeKind::Directory
        );
    }
}
