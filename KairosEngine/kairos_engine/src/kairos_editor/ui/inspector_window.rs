use std::{
    any::{Any, type_name},
    fmt::Debug,
    fs,
    path::PathBuf,
};

use crate::{
    kairos_editor::{
        Engine,
        asset_registry::{AssetKind, Guid},
        ui::{Messager, UIReader, dialog::Dialog, inspector::Inspector},
    },
    kairos_game::KairosGame,
    log::Log,
};
use serde::{Deserialize, Serialize};

use crate::kairos_editor::ui::{Drawer, Message, paths};

#[derive(Debug, Serialize, Deserialize)]
pub struct InspectorWindowStyle {
    pub title: String,
}

/// ProjectWindow 选中节点时传递给 InspectorWindow 的身份信息。
/// 只含路径和类型标识，Inspector 自行从文件读取详细内容。
pub struct InspectorNodeInfo {
    pub name: String,
    pub kind: AssetKind,
    pub path: PathBuf,
    pub guid: Guid,
    pub inspector: Box<dyn Inspector>,
}
impl Debug for InspectorNodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InspectorNodeInfo")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("path", &self.path)
            .finish()
    }
}
impl PartialEq for InspectorNodeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.guid == other.guid
    }
}
impl Eq for InspectorNodeInfo {}

pub struct InspectorWindowModel {
    pub style: InspectorWindowStyle,
    /// 当前选中的节点信息，None 表示无选中
    pub selected: Option<InspectorNodeInfo>,
}

pub struct InspectorWindow {
    model: InspectorWindowModel,
}

impl InspectorWindowStyle {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json =
            fs::read_to_string(paths::PATH_INSPECTOR_WINDOW_STYLE).map_err(|error| {
                format!(
                    "Load InspectorWindow Style Json Failed, path: {}, error: {}",
                    paths::PATH_INSPECTOR_WINDOW_STYLE,
                    error
                )
            })?;
        let style = toml::from_str(&style_json).map_err(|error| {
            format!(
                "Deserialize InspectorWindow Style Json Failed, error: {}",
                error
            )
        })?;
        Ok(style)
    }
}

impl InspectorWindowModel {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style = InspectorWindowStyle::new()?;
        Ok(Self {
            style,
            selected: None,
        })
    }
}

impl InspectorWindow {
    #[inline(always)]
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let model = InspectorWindowModel::new()?;
        Ok(Self { model })
    }

    /// 接收来自 ProjectWindow 的选中节点信息。
    /// 返回旧 Inspector 的退出对话框与待释放的预览 egui texture id（由
    /// Context 统一释放，避免 Inspector 被替换后纹理泄漏）。
    /// 重复选中同一资产时保留现有 Inspector：未保存编辑与预览状态不重建，
    /// 也无需释放纹理。
    pub fn set_selected(
        &mut self,
        ctx: &egui::Context,
        info: Option<InspectorNodeInfo>,
    ) -> (Option<Box<dyn Dialog>>, Vec<egui::TextureId>) {
        if self.model.selected == info {
            return (None, Vec::new());
        }

        let mut dialog = None;
        let mut freed_texture_ids = Vec::new();
        if let Some(selected) = &mut self.model.selected {
            dialog = selected.inspector.on_exit(ctx);
            freed_texture_ids = selected.inspector.take_preview_egui_textures();
        }
        self.model.selected = info;
        (dialog, freed_texture_ids)
    }

    pub fn get_inspector<T: Inspector>(&self) -> Option<&T> {
        self.model
            .selected
            .as_ref()
            .and_then(|info| (info.inspector.as_ref() as &dyn Any).downcast_ref::<T>())
    }

    pub fn get_inspector_mut<T: Inspector>(&mut self) -> Option<&mut T> {
        self.model
            .selected
            .as_mut()
            .and_then(|info| (info.inspector.as_mut() as &mut dyn Any).downcast_mut::<T>())
    }

    pub fn on_close(&mut self, ctx: &egui::Context) -> Vec<egui::TextureId> {
        if let Some(info) = &mut self.model.selected {
            info.inspector.on_exit(ctx);
            return info.inspector.take_preview_egui_textures();
        }
        Vec::new()
    }
}

impl Drawer for InspectorWindow {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Self::new()
    }

    fn show_window(&self, _state: Option<&mut super::docking_tab::window_state::WindowState>) {}

    fn ui(
        &self,
        ui: &mut egui::Ui,
        reader: &UIReader,
        messager: &mut super::Messager,
        engine: &Engine,
        _log: &mut Log,
    ) {
        let Some(info) = &self.model.selected else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a node in Project Window");
            });
            return;
        };

        // ---- header ----
        ui.heading(&info.name);
        ui.label(format!("Kind: {:?}", info.kind));
        ui.separator();

        // ---- common ----
        ui.label(format!("Path: {}", info.path.display()));
        ui.label(format!("GUID: {}", info.guid));
        ui.separator();

        info.inspector.draw(
            ui,
            reader,
            messager,
            &engine.assets_server,
            engine.time.delta_time_secs(),
        );
    }

    fn close(&self, messager: &mut super::Messager) {
        messager.send(Message::CloseInspectorTab);
    }

    fn get_name(&self) -> &'static str {
        type_name::<InspectorWindow>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.model.style.title.to_owned().into()
    }

    fn scroll_bars(&self) -> [bool; 2] {
        [true, false]
    }

    fn get_style_fileds(&self) -> Vec<super::ui_style_fields::StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<super::ui_style_fields::StyleField>) {}

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<crate::graphics::graphics_graph::GraphicsCommand> {
        self.model
            .selected
            .as_ref()
            .and_then(|info| info.inspector.render())
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        asset_loader::assets::AssetsServer,
        kairos_editor::{
            asset_registry::{AssetKind, Guid},
            project_path_tree::ProjectPathGraph,
            ui::{Messager, UIReader},
        },
    };

    /// 记录 on_exit 调用并返回预置 texture id 的桩 Inspector。
    struct StubInspector {
        freed_ids: Vec<egui::TextureId>,
        exit_called: bool,
    }

    impl StubInspector {
        fn new(freed_ids: Vec<egui::TextureId>) -> Self {
            Self {
                freed_ids,
                exit_called: false,
            }
        }
    }

    impl Inspector for StubInspector {
        fn create(
            _path: &std::path::Path,
            _assets_server: &mut AssetsServer,
            _project_graph: &ProjectPathGraph,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            unimplemented!("test stub")
        }

        fn draw(
            &self,
            _ui: &mut egui::Ui,
            _reader: &UIReader,
            _messager: &mut Messager,
            _assets_server: &AssetsServer,
            _dt: f32,
        ) {
        }

        fn on_exit(&mut self, _ctx: &egui::Context) -> Option<Box<dyn Dialog>> {
            self.exit_called = true;
            None
        }

        fn take_preview_egui_textures(&mut self) -> Vec<egui::TextureId> {
            std::mem::take(&mut self.freed_ids)
        }
    }

    fn info_for(guid: Guid, inspector: StubInspector) -> Option<InspectorNodeInfo> {
        Some(InspectorNodeInfo {
            name: "asset".into(),
            kind: AssetKind::Material,
            path: PathBuf::from("res/materials/material.mat"),
            guid,
            inspector: Box::new(inspector),
        })
    }

    fn window_with_selected(guid: Guid, inspector: StubInspector) -> InspectorWindow {
        InspectorWindow {
            model: InspectorWindowModel {
                style: InspectorWindowStyle {
                    title: "Inspector".into(),
                },
                selected: info_for(guid, inspector),
            },
        }
    }

    /// 重复选中同一资产：保留现有 Inspector（未保存编辑 / 预览状态不重建），
    /// 不触发 on_exit、不收集纹理 —— 旧 Inspector 存活，纹理仍归它持有。
    #[test]
    fn set_selected_same_asset_keeps_inspector() {
        let guid = Guid::new();
        let mut window =
            window_with_selected(guid, StubInspector::new(vec![egui::TextureId::User(1)]));
        let ctx = egui::Context::default();

        let (dialog, freed) = window.set_selected(&ctx, info_for(guid, StubInspector::new(vec![])));

        assert!(dialog.is_none());
        assert!(freed.is_empty());
        let inspector = window.get_inspector::<StubInspector>().unwrap();
        assert!(!inspector.exit_called);
        assert_eq!(inspector.freed_ids, vec![egui::TextureId::User(1)]);
    }

    /// 选中不同资产：旧 Inspector 退出并交出其全部预览纹理 id。
    #[test]
    fn set_selected_different_asset_collects_freed_ids() {
        let mut window = window_with_selected(
            Guid::new(),
            StubInspector::new(vec![egui::TextureId::User(1), egui::TextureId::User(2)]),
        );
        let ctx = egui::Context::default();

        let (dialog, freed) =
            window.set_selected(&ctx, info_for(Guid::new(), StubInspector::new(vec![])));

        assert!(dialog.is_none());
        assert_eq!(
            freed,
            vec![egui::TextureId::User(1), egui::TextureId::User(2)]
        );
    }

    /// 取消选中（info = None）：旧 Inspector 的预览纹理 id 同样被收集。
    #[test]
    fn set_selected_none_collects_freed_ids() {
        let mut window = window_with_selected(
            Guid::new(),
            StubInspector::new(vec![egui::TextureId::User(1)]),
        );
        let ctx = egui::Context::default();

        let (dialog, freed) = window.set_selected(&ctx, None);

        assert!(dialog.is_none());
        assert_eq!(freed, vec![egui::TextureId::User(1)]);
        assert!(window.model.selected.is_none());
    }
}
