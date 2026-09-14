pub mod create_request;
pub mod texture_path;
pub mod tree_node;

use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use petgraph::{
    Directed, Graph,
    graph::{Edges, NodeIndex},
    visit::{EdgeRef, NodeIndexable},
};

use crate::{
    asset_registry::{AssetKind, AssetRegistry, AssetRoots, Guid, processed_asset_path},
    dialog,
};
use create_request::CreateRequest;
use tree_node::ProjectTreeNode;

#[cfg(test)]
mod test;

// ============================================================
// ProjectPathGraph — 项目目录树（基于 petgraph）
// ============================================================

/// 以 petgraph 有向图构建的项目目录层级结构。
///
/// 节点权重为 [`ProjectTreeNode`]，边权重为 `()`。
/// 每个节点通过 [`AssetRegistry`] 持有持久化的 GUID。
pub struct ProjectPathGraph {
    pub graph: Graph<ProjectTreeNode, ()>,
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

        let root_path = Self::normalize_path(root_path.to_path_buf());
        let root_guid = registry.get_or_create_guid(&root_path);
        let root_name = root_path
            .canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_os_string()))
            .unwrap_or_else(|| OsString::from("root"));
        let root_node_data = ProjectTreeNode::new(
            root_guid,
            root_name,
            root_path.clone(),
            None,
            AssetKind::Directory,
        );
        let root_node = graph.add_node(root_node_data);

        // root_path 归一化后可能为空（原 `./`），但 read_dir 需要有效路径
        let scan_root = Self::readable_root(&root_path);
        let roots = AssetRoots::new(root_path);
        Self::scan_dir(&scan_root, &roots, root_node, &mut graph, registry);

        Self { graph }
    }

    /// 根节点记录的路径（已归一化；`./` 会归一化为空）。
    fn root_path(&self) -> PathBuf {
        self.graph
            .node_weight(self.get_root_node())
            .map(|node| Self::normalize_path(node.path.clone()))
            .unwrap_or_default()
    }

    /// 树实际扫描的目录：根节点记录的路径，为空时退回 `"."`。
    ///
    /// [`refresh`](Self::refresh) 重新扫描它，编辑器项目树的 watcher 也监听它。
    pub fn scan_root(&self) -> PathBuf {
        Self::readable_root(&self.root_path())
    }

    /// 归一化后的根路径可能为空（原 `./`），但 `read_dir` 需要有效路径。
    fn readable_root(root_path: &Path) -> PathBuf {
        if root_path.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            root_path.to_path_buf()
        }
    }

    /// 刷新整个树（重新扫描）。
    ///
    /// 会保留 Registry 中已有的 GUID，新文件自动注册。
    pub fn refresh(&mut self, registry: &mut AssetRegistry) {
        // The root node's path and the directory it is actually scanned as: the
        // normalized root path can be empty (`./`), which `read_dir` cannot take.
        let root_path = self.root_path();
        let scan_root = self.scan_root();

        self.graph.clear();
        let root_guid = registry.get_or_create_guid(&root_path);
        let root_name = OsString::from("KairosEngine");
        let root_node_data = ProjectTreeNode::new(
            root_guid,
            root_name,
            root_path.clone(),
            None,
            AssetKind::Directory,
        );
        let root_node = self.graph.add_node(root_node_data);

        Self::scan_dir(
            &scan_root,
            &AssetRoots::new(root_path),
            root_node,
            &mut self.graph,
            registry,
        );
    }

    // ----------------------------------------------------------
    // 递归扫描
    // ----------------------------------------------------------

    fn scan_dir(
        dir_path: &Path,
        roots: &AssetRoots,
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
            let path = Self::normalize_path(entry.path());

            // 跳过隐藏文件/目录、target/Library 与成品目录
            if Self::should_skip(&path, roots) {
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
                ProjectTreeNode::new(guid, name, path.clone(), None, AssetKind::Directory);
            let child_node = graph.add_node(node_data);
            graph.add_edge(parent_node, child_node, ());
            Self::scan_dir(&path, roots, child_node, graph, registry);
        }

        // 再处理文件
        for path in files {
            let Some((kind, guid, asset_path)) = registry.analyse_path(&path, roots) else {
                continue;
            };

            let name = path
                .file_prefix()
                .map(|n| n.to_os_string())
                .unwrap_or_default();

            let node_data = ProjectTreeNode::new(guid, name, path.clone(), asset_path, kind);
            let child_node = graph.add_node(node_data);
            graph.add_edge(parent_node, child_node, ());
        }
    }

    /// `parent/{stem}{suffix}`：源文件在主资产、`.meta` 边车等后缀下的兄弟路径。
    fn sibling_path<S: AsRef<OsStr>>(parent: &Path, stem: S, suffix: &str) -> PathBuf {
        let mut file_name = stem.as_ref().to_os_string();
        file_name.push(suffix);
        parent.join(file_name)
    }

    /// 归一化路径：去掉前导 `./`，并将所有分隔符统一为 `/`（POSIX 风格），
    /// 与 `.mat` 等资产文件的内部路径格式保持一致。
    fn normalize_path(path: PathBuf) -> PathBuf {
        let path: PathBuf = path
            .components()
            .filter(|c| !matches!(c, std::path::Component::CurDir))
            .collect();

        // 统一路径分隔符为 `/`，保证跨平台一致性
        let s = path.to_string_lossy().replace('\\', "/");
        PathBuf::from(s)
    }

    /// 判断路径是否应跳过扫描。
    fn should_skip(path: &Path, roots: &AssetRoots) -> bool {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            return false;
        };
        // 跳过隐藏文件/目录
        if name.starts_with('.') {
            return true;
        }
        // 跳过 target 构建目录和 Library 引擎内部目录
        if matches!(name, "target" | "Library") {
            return true;
        }
        // 只跳过源根下顶层的成品目录（如 `imported_assets`）：其内部文件不展示。
        // 同名子目录不受影响；其顶层名字与引擎的 `unprocessed_exclude` 一致。
        let relative = if roots.source.as_os_str().is_empty() {
            path
        } else {
            path.strip_prefix(&roots.source).unwrap_or(path)
        };
        let processed_top = roots
            .processed
            .components()
            .next()
            .map(|c| Path::new(c.as_os_str()));
        if processed_top == Some(relative) {
            return true;
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

    /// 根据相对路径查找节点索引（用于 test harness 定位资产）。
    pub fn find_by_path(&self, path: &Path) -> Option<NodeIndex> {
        // Normalize the search needle to forward slashes for comparison
        let needle = path.to_string_lossy().replace('\\', "/");

        for idx in self.graph.node_indices() {
            if let Some(node) = self.graph.node_weight(idx) {
                let stored = node.path.to_string_lossy().replace('\\', "/");
                // Match if stored path ends with the needle (handles ./ prefix variation)
                if stored.ends_with(&needle) || stored == needle {
                    return Some(idx);
                }
            }
        }
        None
    }

    /// 根据 GUID 查找节点索引。
    ///
    /// 重新扫描会重建所有节点索引（GUID 来自 registry，保持不变），所以跨扫描
    /// 保持选中项时用它定位，见 `ProjectWindow::refresh_from_disk`。
    pub fn find_by_guid(&self, guid: Guid) -> Option<NodeIndex> {
        self.graph
            .node_indices()
            .find(|&idx| self.graph[idx].guid == guid)
    }

    /// 反查引擎资产路径对应的源文件（[`AssetRegistry::analyse_path`] 配对的
    /// 反向）：加工类资产的 `asset_path` 是成品，编辑器要重读源图与它的
    /// `.meta` 时先回到源。没有配对节点时返回 `None`。
    pub fn source_path_for(&self, asset_path: &Path) -> Option<PathBuf> {
        self.graph.node_weights().find_map(|node| {
            (node.asset_path.as_deref() == Some(asset_path)).then(|| node.path.clone())
        })
    }

    /// 按 [`AssetKind`] 过滤所有节点，返回匹配的节点引用。
    ///
    /// 遍历图中所有节点，筛选出 `kind` 与指定类型相匹配的节点。
    /// 用于 MaterialInspector 的 shader 下拉列表等项目树查询场景。
    pub fn find_assets_by_kind(&self, kind: AssetKind) -> Vec<&ProjectTreeNode> {
        self.graph
            .node_weights()
            .filter(|node| node.kind == kind)
            .collect()
    }

    /// 获取节点的所有出边（即子节点列表）。
    pub fn get_edges(&self, node: NodeIndex) -> Edges<'_, (), Directed> {
        self.graph.edges(node)
    }

    /// 获取节点的排序子节点列表：目录优先，同类型按名称排序。
    pub fn sorted_children(&self, node: NodeIndex) -> Vec<(NodeIndex, &ProjectTreeNode)> {
        let mut children: Vec<_> = self
            .get_edges(node)
            .filter_map(|e| self.get_node(e.target()).map(|n| (e.target(), n)))
            .collect();
        children.sort_by(|(_, a), (_, b)| {
            let a_is_dir = a.kind == AssetKind::Directory;
            let b_is_dir = b.kind == AssetKind::Directory;
            if a_is_dir != b_is_dir {
                b_is_dir.cmp(&a_is_dir) // 目录优先
            } else {
                a.name.cmp(&b.name)
            }
        });
        children
    }

    /// 返回图中节点总数。
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn has_child(&self, node: NodeIndex) -> bool {
        self.graph.edges(node).count() > 0
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
        // 验证 base_node 存在于图中
        if self.graph.node_weight(request.base_node).is_none() {
            return None;
        }

        let mut full_path;
        let name;
        let folder_node;
        match request.kind {
            AssetKind::Directory => {
                // ---- 1. 获取parent节点数据 ----
                let Some(folder) = self.get_folder_node(request.base_node) else {
                    dialog::error_message_window(
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
                    dialog::error_message_window(
                        "Create Node Failed",
                        &format!(
                            "failed to create directory '{}': {err}",
                            full_path.display()
                        ),
                    );
                    return None;
                }
            }
            AssetKind::Texture => todo!(),
            AssetKind::Mesh => todo!(),
            AssetKind::Material => todo!(),
            AssetKind::Audio => todo!(),
            AssetKind::Font => todo!(),
            AssetKind::Shader | AssetKind::Script | AssetKind::Document | AssetKind::Toml => {
                let Some(folder) = self.get_folder_node(request.base_node) else {
                    dialog::error_message_window(
                        "Create File Failed",
                        "folder node not found in graph",
                    );
                    return None;
                };
                folder_node = folder.0;
                let folder_data = folder.1;
                name = self.unique_name(folder_node, &request.name);
                full_path = folder_data.path.join(&name);
                if let Some(ext) = request.kind.extension() {
                    full_path.set_extension(ext);
                }
                let content: &str = match request.kind {
                    AssetKind::Shader => Self::default_shader_content(),
                    _ => "",
                };
                if let Err(err) = std::fs::write(&full_path, content) {
                    dialog::error_message_window(
                        "Create File Failed",
                        &format!("failed to create file '{}': {err}", full_path.display()),
                    );
                    return None;
                }
            }
            AssetKind::Unknown => {
                dialog::error_message_window("Create Failed", "Unknown Create Kind");
                return None;
            }
        }

        // ---- 4. 注册 GUID ----
        let guid = registry.get_or_create_guid(&full_path);
        let name = name.into();
        let kind = request.kind;

        // ---- 5. 更新图 ----
        let node_data = ProjectTreeNode::new(guid, name, full_path, None, kind);
        let new_node = self.graph.add_node(node_data);
        self.graph.add_edge(folder_node, new_node, ());

        Some(new_node)
    }

    /// 重命名节点及其关联文件（主资产 + 它的 `.meta` 边车）。
    /// GUID 不变，registry 中的路径同步更新。
    pub fn rename_node(
        &mut self,
        registry: &mut AssetRegistry,
        node: NodeIndex,
        new_name: &str,
    ) -> Result<(), String> {
        let node_data = self.graph.node_weight(node).ok_or("node not found")?;
        let parent = self.get_parent(node).ok_or("cannot rename root")?;

        // 校验同名
        let name_exists = self.get_edges(parent).any(|e| {
            self.get_node(e.target())
                .is_some_and(|n| n.name.to_string_lossy().eq_ignore_ascii_case(new_name))
        });
        if name_exists {
            return Err(format!("'{}' already exists", new_name));
        }

        let old_path = node_data.path.clone();
        let kind = node_data.kind;
        let parent_path = old_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("./"));
        let stem = old_path
            .file_stem()
            .map(|s| s.to_os_string())
            .unwrap_or_default();

        // 收集所有需要重命名的文件路径：主资产 + `.meta` 边车（目录则只有自身）。
        let suffixes = kind.related_suffixes();
        let mut renames: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        if suffixes.is_empty() {
            renames.push((old_path.clone(), parent_path.join(new_name)));
        } else {
            for suffix in &suffixes {
                let old = Self::sibling_path(parent_path, &stem, suffix);
                if old.exists() {
                    renames.push((old, Self::sibling_path(parent_path, new_name, suffix)));
                }
            }
        }

        // 计算新路径并 rename
        for (old, new_path) in &renames {
            std::fs::rename(old, new_path).map_err(|e| {
                format!(
                    "failed to rename '{}' -> '{}': {e}",
                    old.display(),
                    new_path.display()
                )
            })?;
            registry.update_path(old, new_path);
        }

        // 更新 graph 节点；加工类资产重新配对到改名后的成品路径。
        let new_name_os = std::ffi::OsString::from(new_name);
        let new_main_path = match kind.extension() {
            Some(ext) => Self::sibling_path(parent_path, &new_name_os, &format!(".{ext}")),
            None => parent_path.join(&new_name_os),
        };
        let source_root = self
            .graph
            .node_weight(self.get_root_node())
            .map(|n| n.path.clone())
            .unwrap_or_default();
        let new_asset_path = kind
            .is_processed()
            .then(|| processed_asset_path(&new_main_path, &AssetRoots::new(source_root)));

        if let Some(node_weight) = self.graph.node_weight_mut(node) {
            node_weight.name = new_name_os;
            node_weight.path = new_main_path;
            node_weight.asset_path = new_asset_path;
        }

        Ok(())
    }

    /// 删除节点及其关联文件。
    ///
    /// - 文件节点：删除主文件 + 它的 `.meta` 边车
    /// - 目录节点：先递归删除所有子节点，再删除目录本身
    /// - 同步清理 registry
    pub fn delete_node(
        &mut self,
        registry: &mut AssetRegistry,
        node: NodeIndex,
    ) -> Result<(), String> {
        let node_data = self.graph.node_weight(node).ok_or("node not found")?;
        self.get_parent(node).ok_or("cannot delete root")?;

        let is_dir = node_data.kind == AssetKind::Directory;
        let node_path = node_data.path.clone();
        let node_kind = node_data.kind.clone();
        // node_data 引用此后不再使用，后续 delete_node 调用可正常获取 &mut self

        if is_dir {
            // 1. 收集所有子节点（避免 borrow 冲突）
            let children: Vec<NodeIndex> = self.get_edges(node).map(|e| e.target()).collect();

            // 2. 递归删除子节点
            for child in children {
                self.delete_node(registry, child)?;
            }

            // 3. 删除目录（可能含有非 graph 管理的文件，如 .DS_Store）
            let dir_path = node_path.clone();
            std::fs::remove_dir_all(&dir_path)
                .map_err(|e| format!("failed to delete directory '{}': {e}", dir_path.display()))?;
            registry.unregister(&dir_path);
        } else {
            // 1. 收集所有关联文件路径：主资产 + `.meta` 边车
            let suffixes = node_kind.related_suffixes();
            if suffixes.is_empty() {
                let path = node_path.clone();
                if path.exists() {
                    std::fs::remove_file(&path)
                        .map_err(|e| format!("failed to delete '{}': {e}", path.display()))?;
                }
                registry.unregister(&path);
            } else {
                let parent_path = node_path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("./"));
                let stem = node_path
                    .file_stem()
                    .map(|s| s.to_os_string())
                    .unwrap_or_default();
                for suffix in &suffixes {
                    let p = Self::sibling_path(parent_path, &stem, suffix);
                    if p.exists() {
                        std::fs::remove_file(&p)
                            .map_err(|e| format!("failed to delete '{}': {e}", p.display()))?;
                    }
                    registry.unregister(&p);
                }
            }
        }

        // 4. 从 graph 移除节点
        self.graph.remove_node(node);

        Ok(())
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
            Some(data) if data.kind == AssetKind::Directory => Some((node, data)),
            _ => self
                .get_parent(node)
                .and_then(|node| self.get_node(node).map(|data| (node, data))),
        }
    }

    fn default_shader_content() -> &'static str {
        "
// enable f16;
struct InstanceInput {
    @location(5) model_matrix_0: vec4f,
    @location(6) model_matrix_1: vec4f,
    @location(7) model_matrix_2: vec4f,
    @location(8) model_matrix_3: vec4f,
};

struct a2v {
    @location(0) vertex: vec4f,
    @location(1) color: vec4f,
    @location(2) texcoord: vec2f,
    @location(3) normal: vec3f,
    @location(4) tangent: vec4f,
}

struct v2f {
    @builtin(position) pos: vec4f,
    @location(0) color: vec4f,
    @location(1) uv: vec2f,
    @location(2) normal: vec3f,
};

@group(0) @binding(0)
var<uniform> matrix_vp: mat4x4f;

@group(1) @binding(0)
var texture: texture_2d<f32>;
@group(1) @binding(1)
var s_texture: sampler;


@vertex
fn vs_main(v: a2v, instancing: InstanceInput) -> v2f {
    var o: v2f;

    var local_to_world = mat4x4f(
        instancing.model_matrix_0,
        instancing.model_matrix_1,
        instancing.model_matrix_2,
        instancing.model_matrix_3,
    );

    o.pos = matrix_vp * local_to_world * vec4f(v.vertex.xyz, 1.0);
    var normal_world = normalize(local_to_world * vec4f(v.normal.xyz, 0.0));
    o.color = v.color;
    o.uv = v.texcoord;
    o.normal = normal_world.xyz;

    return o;
}

struct gbuffer {
    @location(0) color: vec4f
}

@fragment
fn fs_main(i: v2f) -> gbuffer {
    var out: gbuffer;
    let tex = textureSample(texture, s_texture, i.uv);
    let color = i.color * tex;
    let l = normalize(vec3f(0.0, 1.0, 1.0));
    let ndotl = dot(i.normal, l) * 0.5 + 0.5;
    out.color = color * ndotl;
    // out.color = vec4f(ndotl);
    return out;
}
        "
    }
}
