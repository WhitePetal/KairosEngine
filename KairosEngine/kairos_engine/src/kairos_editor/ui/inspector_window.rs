use std::{any::type_name, cell::Cell, fmt::Debug, fs, path::PathBuf};

use crate::{
    kairos_editor::{
        Engine,
        asset_registry::{AssetKind, Guid},
        ui::{
            Messager,
            global_styles::GlobalStyles,
            inspector::{Inspector},
        },
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
    /// 上一帧指针是否在 Inspector 区域内（用于检测进入/离开）
    pub pointer_was_inside: Cell<bool>,
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
            pointer_was_inside: Cell::new(false),
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
    pub fn set_selected(&mut self, info: Option<InspectorNodeInfo>) {
        if self.model.selected != info && let Some(selected) = &self.model.selected {
            if selected.inspector.dirty() { 
                // TODO: 弹窗提醒是否保存
                log::debug!("TODO: 弹窗提醒是否保存: {:?}", info);
            }
        }
        self.model.selected = info;
    }

    /// 未实现详细 inspector 的文件类型：显示文件元数据
    fn draw_unimpl(ui: &mut egui::Ui, path: &std::path::Path) {
        ui.separator();
        ui.label("not implement inspector");
        match fs::metadata(path) {
            Ok(meta) => {
                ui.label(format!("Size: {} bytes", meta.len()));
            }
            Err(e) => {
                ui.label(format!("Failed to read metadata: {e}"));
            }
        }
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
        _global_styles: &GlobalStyles,
        messager: &mut super::Messager,
        engine: &Engine,
        _log: &mut Log,
    ) {
        // 检测指针进入/离开 Inspector 区域，发送锁定/解锁消息
        let now_inside = ui.rect_contains_pointer(ui.max_rect());
        let was_inside = self.model.pointer_was_inside.get();
        if now_inside != was_inside {
            messager.send(Message::LockProjectSelection(now_inside));
            self.model.pointer_was_inside.set(now_inside);
        }

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

        info.inspector.draw(ui, messager, &engine.assets_server);
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
        None
    }
}
