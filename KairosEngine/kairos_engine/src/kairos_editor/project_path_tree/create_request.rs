use petgraph::graph::NodeIndex;

use super::tree_node::ProjectNodeKind;

// ============================================================
// CreateRequest — 创建请求
// ============================================================

/// 创建节点的请求参数。
#[derive(Debug, Clone)]
pub struct CreateRequest {
    /// 创建时选中/所在的节点
    pub base_node: NodeIndex,
    /// 节点名称（不含路径前缀，如 "new_folder" 或 "readme.md"）
    pub name: String,
    /// 节点类型（Directory / Document / Script / Material / ...）
    pub kind: ProjectNodeKind,
}
