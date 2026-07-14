use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use toml::from_str;

use crate::kairos_editor::{
    asset_registry::AssetKind, project_path_tree::tree_node::ProjectTreeNode, ui::paths,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalStyles {
    pub fonts: FontsConfig,
    pub project_node_icons: ProjectNodeIcons,
}

impl GlobalStyles {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let style_json = fs::read_to_string(paths::PATH_GLOABL_STYLE).map_err(|error| {
            format!(
                "Load Global Style Toml Failed, path: {}, error: {}",
                paths::PATH_GLOABL_STYLE,
                error
            )
        })?;
        let style: Self = from_str(&style_json)
            .map_err(|error| format!("Deserialize Global Style Toml Failed, error: {}", error))?;
        Ok(style)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectNodeIcons {
    pub default: String,
    pub directory: Option<String>,
    pub directory_fill: Option<String>,
    pub mesh: Option<String>,
    pub material: Option<String>,
    pub audio: Option<String>,
    pub shader: Option<String>,
    pub script: Option<String>,
    pub document: Option<String>,
    pub toml: Option<String>,
    pub font: Option<String>,
}

fn default_icon_path() -> String {
    paths::PATH_ENGINE_ICON.into()
}

impl ProjectNodeIcons {
    /// 根据节点类型获取对应图标路径，未配置则回退到 `default`。
    pub fn for_kind<'a>(&'a self, node: &'a ProjectTreeNode, has_child: bool) -> &'a str {
        let opt = match node.kind {
            AssetKind::Directory => {
                if has_child {
                    self.directory_fill.as_deref()
                } else {
                    self.directory.as_deref()
                }
            }
            AssetKind::Texture => node.path.to_str(),
            AssetKind::Mesh => self.mesh.as_deref(),
            AssetKind::Material => self.material.as_deref(),
            AssetKind::Audio => self.audio.as_deref(),
            AssetKind::Shader => self.shader.as_deref(),
            AssetKind::Script => self.script.as_deref(),
            AssetKind::Document => self.document.as_deref(),
            AssetKind::Toml => self.toml.as_deref(),
            AssetKind::Font => self.font.as_deref(),
            AssetKind::Unknown => None,
        };
        opt.unwrap_or(&self.default)
    }

    /// 将相对图标路径转为 `file://` URI（跨平台）。
    ///
    /// Windows 上 `file://rel/path` 会被 `egui_extras::FileLoader` 误解析为 UNC，
    /// 因此必须用 `std::path::absolute` 先转为绝对路径再构造 URI。
    pub fn uri_for_kind(&self, node: &ProjectTreeNode, has_child: bool) -> String {
        let relative = self.for_kind(node, has_child);
        let abs_path = std::path::absolute(relative).unwrap_or_else(|_| PathBuf::from(relative));

        #[cfg(target_os = "windows")]
        {
            let s = abs_path.display().to_string().replace('\\', "/");
            format!("file:///{}", s)
        }
        #[cfg(not(target_os = "windows"))]
        {
            format!("file://{}", abs_path.display())
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum FontPriority {
    First,
    Push,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FontDataConfig {
    pub name: String,
    pub path: PathBuf,
    pub priority: FontPriority,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FontsConfig {
    pub proportional: Vec<FontDataConfig>,
    pub monospace: Vec<FontDataConfig>,
}
