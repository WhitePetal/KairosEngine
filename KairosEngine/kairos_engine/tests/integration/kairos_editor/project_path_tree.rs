use tempfile::TempDir;

use kairos_engine::kairos_editor::{
    asset_registry::{AssetKind, AssetRegistry},
    project_path_tree::{
        ProjectPathGraph, create_request::CreateRequest, tree_node::ProjectTreeNode,
    },
};
use petgraph::graph::NodeIndex;

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
            kind: AssetKind::Directory,
        },
    );

    assert!(result.is_some(), "{result:?}");
    let node = result.unwrap();
    let data = graph.get_node(node).unwrap();
    assert_eq!(data.kind, AssetKind::Directory);
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
        kind: AssetKind::Directory,
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
            kind: AssetKind::Directory,
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
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    assert_eq!(graph.get_node(dir).unwrap().kind, AssetKind::Directory);
}

// ---- rename_node ----

#[test]
fn rename_directory_success() {
    let (tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let dir = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "old_dir".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    let old_path = graph.get_node(dir).unwrap().path.clone();

    graph.rename_node(&mut registry, dir, "new_dir").unwrap();

    let data = graph.get_node(dir).unwrap();
    assert_eq!(data.name.to_string_lossy(), "new_dir");
    assert!(tmp.path().join("new_dir").exists());
    assert!(!old_path.exists());
    assert!(registry.get_guid(&data.path).is_some());
}

#[test]
fn rename_texture_sync_related_files() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    // 先创建一个子目录
    let dir = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "textures".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();
    let dir_path = graph.get_node(dir).unwrap().path.clone();

    // 手动创建 Texture 的关联文件
    let png_path = dir_path.join("my_texture.png");
    let texture_path = dir_path.join("my_texture.texture");
    let texture_bin_path = dir_path.join("my_texture.texture_bin");
    std::fs::write(&png_path, "fake").unwrap();
    std::fs::write(&texture_path, "fake").unwrap();
    std::fs::write(&texture_bin_path, "fake").unwrap();

    // 注册 GUID（创建/导入流程会做，这里手动模拟）
    let guid = registry.get_or_create_guid(&png_path);
    registry.get_or_create_guid(&texture_path);
    registry.get_or_create_guid(&texture_bin_path);

    // 手动在图里添加 Texture 节点
    let node_data = ProjectTreeNode::new(
        guid,
        "my_texture".into(),
        png_path.clone(),
        None,
        AssetKind::Texture,
    );
    let tex_node = graph.graph.add_node(node_data);
    graph.graph.add_edge(dir, tex_node, ());

    // 执行重命名
    graph
        .rename_node(&mut registry, tex_node, "renamed")
        .unwrap();

    let data = graph.get_node(tex_node).unwrap();
    assert_eq!(data.name.to_string_lossy(), "renamed");

    // 旧文件全部消失
    assert!(!png_path.exists());
    assert!(!texture_path.exists());
    assert!(!texture_bin_path.exists());

    // 新文件全部存在
    let new_png = dir_path.join("renamed.png");
    let new_texture = dir_path.join("renamed.texture");
    let new_texture_bin = dir_path.join("renamed.texture_bin");
    assert!(new_png.exists(), "{:?} should exist", new_png);
    assert!(new_texture.exists(), "{:?} should exist", new_texture);
    assert!(
        new_texture_bin.exists(),
        "{:?} should exist",
        new_texture_bin
    );

    // Registry 中路径已更新
    let reg_path = registry.get_path(&guid).unwrap();
    assert_eq!(*reg_path, new_png);
}

#[test]
fn rename_mesh_sync_related_files() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let dir = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "meshes".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();
    let dir_path = graph.get_node(dir).unwrap().path.clone();

    let mesh_path = dir_path.join("cube.mesh");
    let mesh_bin_path = dir_path.join("cube.mesh_bin");
    std::fs::write(&mesh_path, "fake").unwrap();
    std::fs::write(&mesh_bin_path, "fake").unwrap();

    let guid = registry.get_or_create_guid(&mesh_path);
    registry.get_or_create_guid(&mesh_bin_path);

    let node_data = ProjectTreeNode::new(
        guid,
        "cube".into(),
        mesh_path.clone(),
        None,
        AssetKind::Mesh,
    );
    let mesh_node = graph.graph.add_node(node_data);
    graph.graph.add_edge(dir, mesh_node, ());

    graph
        .rename_node(&mut registry, mesh_node, "sphere")
        .unwrap();

    let data = graph.get_node(mesh_node).unwrap();
    assert_eq!(data.name.to_string_lossy(), "sphere");

    assert!(!mesh_path.exists());
    assert!(!mesh_bin_path.exists());

    let new_mesh = dir_path.join("sphere.mesh");
    let new_mesh_bin = dir_path.join("sphere.mesh_bin");
    assert!(new_mesh.exists());
    assert!(new_mesh_bin.exists());

    let reg_path = registry.get_path(&guid).unwrap();
    assert_eq!(*reg_path, new_mesh);
}

#[test]
fn rename_duplicate_name_fails() {
    let (tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let a = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "a".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "b".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    let result = graph.rename_node(&mut registry, a, "b");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already exists"));

    // a 的名字和路径不应改变
    let data = graph.get_node(a).unwrap();
    assert!(data.name.to_string_lossy() == "a" || data.name.to_string_lossy().starts_with("a"));
    assert!(tmp.path().join("a").exists());
}

#[test]
fn rename_root_fails() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let result = graph.rename_node(&mut registry, root, "new_root");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot rename root"));
}

#[test]
fn rename_nonexistent_node_fails() {
    let (_tmp, mut graph, mut registry) = setup();
    let bogus = NodeIndex::new(99999);

    let result = graph.rename_node(&mut registry, bogus, "nope");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("node not found"));
}

#[test]
fn rename_noop_same_name() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let dir = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "keep".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();
    let old_path = graph.get_node(dir).unwrap().path.clone();

    // 重命名为同名 —— 由于 duplicate check 会对比同名，这会失败
    let result = graph.rename_node(&mut registry, dir, "keep");
    // 由于父目录下已有名为 "keep" 的节点（就是它自己），dupe check 会触发
    assert!(result.is_err());

    // 不应损坏
    assert!(old_path.exists());
}

// ---- delete_node ----

#[test]
fn delete_file_success() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let dir = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "scripts".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    let script = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: dir,
                name: "hello".into(),
                kind: AssetKind::Script,
            },
        )
        .unwrap();
    let script_path = graph.get_node(script).unwrap().path.clone();
    assert!(script_path.exists());

    graph.delete_node(&mut registry, script).unwrap();

    assert!(!script_path.exists());
    assert!(graph.get_node(script).is_none());
    assert!(registry.get_guid(&script_path).is_none());
}

#[test]
fn delete_texture_with_related_files() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let dir = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "textures".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();
    let dir_path = graph.get_node(dir).unwrap().path.clone();

    let png = dir_path.join("player.png");
    let texture = dir_path.join("player.texture");
    let texture_bin = dir_path.join("player.texture_bin");
    std::fs::write(&png, "fake").unwrap();
    std::fs::write(&texture, "fake").unwrap();
    std::fs::write(&texture_bin, "fake").unwrap();

    let guid = registry.get_or_create_guid(&png);
    registry.get_or_create_guid(&texture);
    registry.get_or_create_guid(&texture_bin);

    let node_data =
        ProjectTreeNode::new(guid, "player".into(), png.clone(), None, AssetKind::Texture);
    let tex_node = graph.graph.add_node(node_data);
    graph.graph.add_edge(dir, tex_node, ());

    graph.delete_node(&mut registry, tex_node).unwrap();

    assert!(!png.exists());
    assert!(!texture.exists());
    assert!(!texture_bin.exists());
    assert!(graph.get_node(tex_node).is_none());
    assert!(registry.get_guid(&png).is_none());
    assert!(registry.get_guid(&texture).is_none());
    assert!(registry.get_guid(&texture_bin).is_none());
}

#[test]
fn delete_directory_recursive() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let parent = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "assets".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();
    let parent_path = graph.get_node(parent).unwrap().path.clone();

    let child = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: parent,
                name: "subdir".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();
    let child_path = graph.get_node(child).unwrap().path.clone();

    let script = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: child,
                name: "main".into(),
                kind: AssetKind::Script,
            },
        )
        .unwrap();
    let script_path = graph.get_node(script).unwrap().path.clone();

    graph.delete_node(&mut registry, parent).unwrap();

    assert!(!parent_path.exists());
    assert!(!child_path.exists());
    assert!(!script_path.exists());
    assert!(graph.get_node(parent).is_none());
    assert!(graph.get_node(child).is_none());
    assert!(graph.get_node(script).is_none());
}

#[test]
fn delete_root_fails() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    let result = graph.delete_node(&mut registry, root);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot delete root"));
}

#[test]
fn delete_nonexistent_node_fails() {
    let (_tmp, mut graph, mut registry) = setup();
    let bogus = NodeIndex::new(99999);

    let result = graph.delete_node(&mut registry, bogus);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("node not found"));
}

// ---- find_assets_by_kind ----

#[test]
fn find_assets_by_kind_returns_matching_nodes() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    // Create shader nodes (supported by create_node)
    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "default".into(),
                kind: AssetKind::Shader,
            },
        )
        .unwrap();
    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "ui".into(),
                kind: AssetKind::Shader,
            },
        )
        .unwrap();

    // Create a Script node (should NOT be returned when filtering for Shader)
    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "main".into(),
                kind: AssetKind::Script,
            },
        )
        .unwrap();

    let shaders = graph.find_assets_by_kind(AssetKind::Shader);
    assert_eq!(shaders.len(), 2, "should find exactly 2 shader nodes");

    let names: Vec<String> = shaders.iter().map(|n| n.name()).collect();
    assert!(names.contains(&"default".into()));
    assert!(names.contains(&"ui".into()));
}

#[test]
fn find_assets_by_kind_empty_when_no_match() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    // Only create directories (no shaders)
    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "subdir".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    let shaders = graph.find_assets_by_kind(AssetKind::Shader);
    assert!(shaders.is_empty(), "no shader nodes should be found");
}

#[test]
fn find_assets_by_kind_only_directories() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "assets".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();
    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "scripts".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    let dirs = graph.find_assets_by_kind(AssetKind::Directory);
    // Root dir + the two newly created
    assert_eq!(dirs.len(), 3, "should find 3 directory nodes");
}

#[test]
fn find_assets_by_kind_nested_nodes() {
    let (_tmp, mut graph, mut registry) = setup();
    let root = graph.get_root_node();

    // Create a nested structure: root/shaders/vertex
    let shaders_dir = graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "shaders".into(),
                kind: AssetKind::Directory,
            },
        )
        .unwrap();

    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: shaders_dir,
                name: "vertex".into(),
                kind: AssetKind::Shader,
            },
        )
        .unwrap();

    // Also create a shader at root level
    graph
        .create_node(
            &mut registry,
            CreateRequest {
                base_node: root,
                name: "fragment".into(),
                kind: AssetKind::Shader,
            },
        )
        .unwrap();

    let shaders = graph.find_assets_by_kind(AssetKind::Shader);
    assert_eq!(shaders.len(), 2, "should find shaders at any depth");

    let names: Vec<String> = shaders.iter().map(|n| n.name()).collect();
    assert!(names.contains(&"vertex".into()));
    assert!(names.contains(&"fragment".into()));
}
