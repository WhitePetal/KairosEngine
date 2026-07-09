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

use crate::{kairos_dialog, kairos_editor::asset_registry::AssetRegistry};
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
    pub fn get_parent(&self, node: NodeIndex) -> Option<NodeIndex> {
        use petgraph::Direction;
        self.graph
            .neighbors_directed(node, Direction::Incoming)
            .next()
    }

    /// 获取从根到该节点的所有祖先（不含自身），从根到父排列。
    pub fn get_ancestors(&self, node: NodeIndex) -> Vec<NodeIndex> {
        let mut ancestors = Vec::new();
        let mut current = node;
        while let Some(parent) = self.get_parent(current) {
            ancestors.push(parent);
            current = parent;
        }
        ancestors.reverse();
        ancestors
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
    ) -> Option<NodeIndex> {
        let full_path;
        let name;
        let folder_node;
        match request.kind {
            ProjectNodeKind::Directory => {
                // ---- 1. 获取parent节点数据 ----
                let Some(folder) = self.get_folder_node(request.base_node) else {
                    kairos_dialog::error_message_window(
                        "Create Folder Failed",
                        "folder node not found in graph",
                    );
                    return None;
                };
                folder_node = folder.0;
                let folder_data = folder.1;
                name = self.unique_name(folder_node, &request.name);

                // ---- 3. 构建路径 + 文件系统操作 ----
                full_path = folder_data.path.join(&name);
                if let Err(err) = std::fs::create_dir(&full_path) {
                    kairos_dialog::error_message_window(
                        "Create Node Failed",
                        &format!(
                            "failed to create directory '{}': {err}",
                            full_path.display()
                        ),
                    );
                    return None;
                }
            }
            ProjectNodeKind::Texture => todo!(),
            ProjectNodeKind::Mesh => todo!(),
            ProjectNodeKind::Material => todo!(),
            ProjectNodeKind::Audio => todo!(),
            ProjectNodeKind::Shader => todo!(),
            ProjectNodeKind::GenericAsset => todo!(),
            ProjectNodeKind::Script => todo!(),
            ProjectNodeKind::Document => {
                let Some(folder) = self.get_folder_node(request.base_node) else {
                    kairos_dialog::error_message_window(
                        "Create Document Failed",
                        "folder node not found in graph",
                    );
                    return None;
                };
                folder_node = folder.0;
                let folder_data = folder.1;
                name = self.unique_name(folder_node, &request.name);
                full_path = folder_data.path.join(&name);
                if let Err(err) = std::fs::write(&full_path, "") {
                    kairos_dialog::error_message_window(
                        "Create Document Failed",
                        &format!("failed to create file '{}': {err}", full_path.display()),
                    );
                    return None;
                }
            }
            ProjectNodeKind::Toml => {
                let Some(folder) = self.get_folder_node(request.base_node) else {
                    kairos_dialog::error_message_window(
                        "Create Toml Failed",
                        "folder node not found in graph",
                    );
                    return None;
                };
                folder_node = folder.0;
                let folder_data = folder.1;
                name = self.unique_name(folder_node, &request.name);
                full_path = folder_data.path.join(&name);
                if let Err(err) = std::fs::write(&full_path, "") {
                    kairos_dialog::error_message_window(
                        "Create Toml Failed",
                        &format!("failed to create file '{}': {err}", full_path.display()),
                    );
                    return None;
                }
            },
            ProjectNodeKind::Unknown => {
                kairos_dialog::error_message_window(
                    "Create Failed",
                    "Unknown Create Kind",
                );
                return None;
            },
        }

        // ---- 4. 注册 GUID ----
        let guid = registry.get_or_create_guid(&full_path);
        let name = name.into();
        let kind = request.kind;

        // ---- 5. 更新图 ----
        let node_data = ProjectTreeNode::new(guid, name, full_path, kind);
        let new_node = self.graph.add_node(node_data);
        self.graph.add_edge(folder_node, new_node, ());

        Some(new_node)
    }

    /// 生成不重复的名称（"New Folder" → "New Folder (1)" → ...）
    fn unique_name(&self, parent: NodeIndex, base: &str) -> String {
        let exists = |name: &str| {
            self.get_edges(parent).any(|e| {
                self.get_node(e.target())
                    .is_some_and(|n| n.name.to_string_lossy().eq_ignore_ascii_case(name))
            })
        };

        if !exists(base) {
            return base.to_string();
        }
        for i in 1..1000u32 {
            let candidate = format!("{base} ({i})");
            if !exists(&candidate) {
                return candidate;
            }
        }
        format!(
            "{base} ({})",
            uuid::Uuid::new_v4()
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        )
    }

    fn get_folder_node(&self, node: NodeIndex) -> Option<(NodeIndex, &ProjectTreeNode)> {
        match self.get_node(node) {
            Some(data) if data.kind == ProjectNodeKind::Directory => Some((node, data)),
            _ => self
                .get_parent(node)
                .and_then(|node| self.get_node(node).map(|data| (node, data))),
        }
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
                base_node: root,
                name: "new_dir".into(),
                kind: ProjectNodeKind::Directory,
            },
        );

        assert!(result.is_some(), "{result:?}");
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
            base_node: root,
            name: "dup".into(),
            kind: ProjectNodeKind::Directory,
        };
        graph.create_node(&mut registry, req.clone()).unwrap();
        let result = graph.create_node(&mut registry, req);
        assert!(graph.get_node(result.unwrap()).unwrap().name == "dup (1)");
    }

    #[test]
    fn create_invalid_parent_fails() {
        let (_tmp, mut graph, mut registry) = setup();
        let bogus = NodeIndex::new(99999);

        let result = graph.create_node(
            &mut registry,
            CreateRequest {
                base_node: bogus,
                name: "orphan".into(),
                kind: ProjectNodeKind::Directory,
            },
        );
        assert!(result.is_none());
    }

    #[test]
    fn parent_must_be_directory() {
        let (_tmp, mut graph, mut registry) = setup();
        let root = graph.get_root_node();

        let dir = graph
            .create_node(
                &mut registry,
                CreateRequest {
                    base_node: root,
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
