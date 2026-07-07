use egui::{CollapsingHeader, Color32, RichText, Vec2};
use petgraph::{graph::NodeIndex, visit::EdgeRef};

use crate::kairos_editor::{
    asset_registry::AssetRegistry,
    project_path_tree::{
        ProjectPathGraph,
        tree_node::{ProjectNodeKind, ProjectTreeNode},
    },
};

// ============================================================
// 常量
// ============================================================

/// 临时图标路径（后续替换为各类型的专属图标）
const TEMP_ICON_PATH: &str = "file://res/textures/kairos_texture.png";

/// 图标显示尺寸
const ICON_SIZE: Vec2 = Vec2 { x: 18.0, y: 18.0 };

/// 文件夹箭头展开/折叠颜色
#[allow(dead_code)]
const ARROW_COLOR: Color32 = Color32::from_rgb(160, 160, 160);

/// 目录名颜色
const DIR_COLOR: Color32 = Color32::from_rgb(220, 220, 180);

/// 文件名颜色
const FILE_COLOR: Color32 = Color32::from_rgb(200, 200, 200);

// ============================================================
// Hierarchy 面板入口
// ============================================================

/// 在 egui Ui 中绘制项目目录树（Hierarchy 面板）。
pub fn draw(ui: &mut egui::Ui, graph: &ProjectPathGraph, _registry: &AssetRegistry) {
    egui::ScrollArea::both().show(ui, |ui| {
        let root = graph.get_root_node();
        draw_node(ui, graph, root);
    });
}

// ============================================================
// 递归渲染
// ============================================================

fn draw_node(ui: &mut egui::Ui, graph: &ProjectPathGraph, node: NodeIndex) {
    let Some(node_data) = graph.get_node(node) else {
        return;
    };

    match &node_data.kind {
        ProjectNodeKind::Directory => draw_directory(ui, graph, node, node_data),
        _ => draw_file(ui, node_data),
    }
}

/// 绘制目录节点 — 使用 CollapsingHeader，内部递归渲染子节点。
fn draw_directory(
    ui: &mut egui::Ui,
    graph: &ProjectPathGraph,
    node: NodeIndex,
    node_data: &ProjectTreeNode,
) {
    let name = node_name(node_data);
    let header_text = RichText::new(name).color(DIR_COLOR);

    // 使用 GUID 字符串作为 CollapsingHeader 的 Id，保证跨帧状态稳定
    let header = CollapsingHeader::new(header_text).id_salt(node_data.guid.to_string());

    // 检查是否有子节点
    let has_children = graph.get_edges(node).count() > 0;

    if has_children {
        header.show(ui, |ui| {
            let edges: Vec<_> = graph.get_edges(node).map(|e| e.target()).collect();
            for child in edges {
                draw_node(ui, graph, child);
            }
        });
    } else {
        // 空目录也用 CollapsingHeader 展示（无 body），保持视觉一致性
        header.show(ui, |_ui| {});
    }
}

/// 绘制文件（资源）节点 — 图标 + 文件名。
fn draw_file(ui: &mut egui::Ui, node_data: &ProjectTreeNode) {
    ui.horizontal(|ui| {
        // 图标
        let icon = egui::Image::new(egui::ImageSource::Uri(TEMP_ICON_PATH.into()))
            .fit_to_exact_size(ICON_SIZE);
        ui.add(icon);

        // 文件名
        let name = node_name(node_data);
        let label_text = RichText::new(name).color(FILE_COLOR);
        ui.label(label_text);

        // 类型后缀（灰色小字）
        if let Some(suffix) = kind_suffix(&node_data.kind) {
            ui.label(
                RichText::new(suffix)
                    .size(11.0)
                    .color(Color32::from_rgb(120, 120, 120)),
            );
        }
    });
}

// ============================================================
// Helpers
// ============================================================

/// 从节点数据提取显示名称。
fn node_name(node_data: &ProjectTreeNode) -> String {
    node_data.name.to_string_lossy().into_owned()
}

/// 资源类型的后缀标签（如 ".texture"、".mesh"）。
fn kind_suffix(kind: &ProjectNodeKind) -> Option<&'static str> {
    match kind {
        ProjectNodeKind::Directory => None,
        ProjectNodeKind::Texture => Some(".texture"),
        ProjectNodeKind::Mesh => Some(".mesh"),
        ProjectNodeKind::Material => Some(".mat"),
        ProjectNodeKind::Audio => Some(".audio"),
        ProjectNodeKind::Shader => Some(".wgsl"),
        ProjectNodeKind::GenericAsset => Some(".asset"),
        ProjectNodeKind::Script => Some(".rs"),
        ProjectNodeKind::Document => None, // .md/.txt 不加后缀，保持简洁
        ProjectNodeKind::Unknown => None,
    }
}
