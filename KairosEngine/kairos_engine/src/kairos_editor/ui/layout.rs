use std::any::type_name;

use crate::{
    graphics::graphics_graph::GraphicsCommand,
    kairos_editor::{
        Engine,
        ui::{Drawer, Messager, global_styles::GlobalStyles},
    },
    kairos_game::KairosGame,
    log::Log,
};

use super::{
    docking_tab::{
        dock_state::{
            DockState,
            tree::{NodeIndex, Split, node::Node},
        },
        surfaces::SurfaceIndex,
        window_state::WindowState,
    },
    ui_style_fields::StyleField,
};

pub struct LayoutLeftContainer {}
pub struct LayoutRightContainer {}
pub struct LayoutBottomContainer {}
pub struct LayoutCenterContainer {}

impl Drawer for LayoutLeftContainer {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn show_window(&self, _state: Option<&mut WindowState>) {}

    fn ui(
        &self,
        _ui: &mut egui::Ui,
        _global_styles: &GlobalStyles,
        _messager: &mut Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
    }

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<GraphicsCommand> {
        None
    }

    fn close(&self, _messager: &mut Messager) {}

    fn scroll_bars(&self) -> [bool; 2] {
        [false, false]
    }

    fn get_name(&self) -> &'static str {
        // 返回带容器标识的名字，避免和真实窗口重名
        type_name::<LayoutLeftContainer>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.get_name().into()
    }

    fn get_style_fileds(&self) -> Vec<StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<StyleField>) {}
}
impl Drawer for LayoutRightContainer {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn show_window(&self, _state: Option<&mut WindowState>) {}

    fn ui(
        &self,
        _ui: &mut egui::Ui,
        _global_styles: &GlobalStyles,
        _messager: &mut Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
    }

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<GraphicsCommand> {
        None
    }

    fn close(&self, _messager: &mut Messager) {}

    fn get_name(&self) -> &'static str {
        type_name::<LayoutRightContainer>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.get_name().into()
    }

    fn get_style_fileds(&self) -> Vec<StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<StyleField>) {}
}
impl Drawer for LayoutBottomContainer {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn show_window(&self, _state: Option<&mut WindowState>) {}

    fn ui(
        &self,
        _ui: &mut egui::Ui,
        _global_styles: &GlobalStyles,
        _messager: &mut Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
    }

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<GraphicsCommand> {
        None
    }

    fn close(&self, _messager: &mut Messager) {}

    fn get_name(&self) -> &'static str {
        type_name::<LayoutBottomContainer>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.get_name().into()
    }

    fn get_style_fileds(&self) -> Vec<StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<StyleField>) {}
}
impl Drawer for LayoutCenterContainer {
    fn create(
        _assets_server: &mut crate::asset_loader::assets::AssetsServer,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn show_window(&self, _state: Option<&mut WindowState>) {}

    fn ui(
        &self,
        _ui: &mut egui::Ui,
        _global_styles: &GlobalStyles,
        _messager: &mut Messager,
        _engine: &Engine,
        _log: &mut Log,
    ) {
    }

    fn render(
        &self,
        _engine: &mut Engine,
        _game: &mut KairosGame,
        _messager: &mut Messager,
    ) -> Option<GraphicsCommand> {
        None
    }

    fn close(&self, _messager: &mut Messager) {}

    fn get_name(&self) -> &'static str {
        type_name::<LayoutCenterContainer>()
    }

    fn get_title(&self) -> egui::WidgetText {
        self.get_name().into()
    }

    fn get_style_fileds(&self) -> Vec<StyleField> {
        Vec::new()
    }

    fn update_style(&mut self, _style_fields: &Vec<StyleField>) {}
}

// ============================================================
// Zone / EditorLayout
// ============================================================

#[derive(Debug, Clone, Copy)]
pub struct Zone {
    pub surface: SurfaceIndex,
    pub node: NodeIndex,
}

pub struct EditorLayout {
    pub left: Zone,
    pub center: Zone,
    pub right: Zone,
    pub bottom: Zone,
}

pub struct LayoutContainerIds {
    pub left: usize,
    pub right: usize,
    pub bottom: usize,
}

impl EditorLayout {
    /// 用 4 个容器 drawer ID 创建 Unity 风格布局骨架
    pub fn new() -> Self {
        Self {
            left: Zone {
                surface: SurfaceIndex::main(),
                node: NodeIndex::root(),
            },
            center: Zone {
                surface: SurfaceIndex::main(),
                node: NodeIndex::root(),
            },
            right: Zone {
                surface: SurfaceIndex::main(),
                node: NodeIndex::root(),
            },
            bottom: Zone {
                surface: SurfaceIndex::main(),
                node: NodeIndex::root(),
            },
        }
    }

    pub fn init_layout(&mut self, dock_state: &mut DockState<usize>, ids: LayoutContainerIds) {
        let surface = SurfaceIndex::main();

        // Step 1: 右侧 Inspector (20%) — new 放右边
        let [main_area, right_node] = dock_state[surface].split(
            NodeIndex::root(),
            Split::Right,
            0.8,
            Node::leaf_with(vec![ids.right]),
        );

        // Step 2: 底部区域 (30%) — new 放下方
        let [center_area, bottom_node] = dock_state[surface].split(
            main_area,
            Split::Below,
            0.7,
            Node::leaf_with(vec![ids.bottom]),
        );

        // Step 3: 左侧 Hierarchy (25%) — new 放左边
        let [center_node, left_node] = dock_state[surface].split(
            center_area,
            Split::Left,
            0.25,
            Node::leaf_with(vec![ids.left]),
        );

        self.left = Zone {
            surface,
            node: left_node,
        };
        self.center = Zone {
            surface,
            node: center_node,
        };
        self.right = Zone {
            surface,
            node: right_node,
        };
        self.bottom = Zone {
            surface,
            node: bottom_node,
        };
    }
}
