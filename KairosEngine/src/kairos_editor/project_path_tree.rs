pub mod texture_path;

use std::path::{Path, PathBuf};

use petgraph::{
    Directed, Graph,
    graph::{Edges, NodeIndex},
    visit::NodeIndexable,
};

use crate::kairos_editor::project_path_tree::texture_path::TexturePath;

pub enum ProjectPath {
    Dir(PathBuf),
    Texture(TexturePath),
    Asset(PathBuf),
}

pub struct ProjectPathGraph {
    graph: Graph<ProjectPath, ()>,
}

impl ProjectPathGraph {
    pub fn new() -> Self {
        let mut graph = Graph::new();
        let path = Path::new("./");
        let root_node = graph.add_node(ProjectPath::Dir(path.into()));
        Self::scan_project_dirs(&path, root_node, &mut graph);
        Self { graph }
    }

    fn scan_project_dirs(path: &Path, node: NodeIndex, graph: &mut Graph<ProjectPath, ()>) {
        let Some(read_dir) = std::fs::read_dir(path).ok() else {
            return;
        };

        for entry in read_dir {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_dir() {
                        let pp = ProjectPath::Dir(path.clone());
                        let leaf = graph.add_node(pp);
                        graph.add_edge(node, leaf, ());
                        Self::scan_project_dirs(&path, leaf, graph);
                    } else {
                        let pp = {
                            match path.extension().and_then(|ext| ext.to_str()) {
                                Some("texture") => {
                                    let file_name = path.file_name().unwrap().to_owned();
                                    let tex = TexturePath::new(path.clone(), path, file_name);
                                    Some(ProjectPath::Texture(tex))
                                }
                                Some("asset") => Some(ProjectPath::Asset(path)),
                                _ => None,
                            }
                        };
                        if let Some(pp) = pp {
                            let leaf = graph.add_node(pp);
                            graph.add_edge(node, leaf, ());
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }

    pub fn get_root_node(&self) -> NodeIndex {
        self.graph.from_index(0)
    }

    pub fn get_path(&self, node: NodeIndex) -> Option<&ProjectPath> {
        self.graph.node_weight(node)
    }

    pub fn get_edges(&self, node: NodeIndex) -> Edges<'_, (), Directed> {
        self.graph.edges(node)
    }
}
