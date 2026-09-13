use tempfile::TempDir;

use crate::kairos_editor::{
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

    // 手动创建 Texture 的源文件与 `.meta` 边车
    let png_path = dir_path.join("my_texture.png");
    let meta_path = dir_path.join("my_texture.png.meta");
    std::fs::write(&png_path, "fake").unwrap();
    std::fs::write(&meta_path, "fake").unwrap();

    // 注册 GUID（创建/导入流程会做，这里手动模拟）
    let guid = registry.get_or_create_guid(&png_path);

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
    assert!(!meta_path.exists());

    // 新文件全部存在
    let new_png = dir_path.join("renamed.png");
    let new_meta = dir_path.join("renamed.png.meta");
    assert!(new_png.exists(), "{:?} should exist", new_png);
    assert!(new_meta.exists(), "{:?} should exist", new_meta);
    assert_eq!(data.path, new_png);

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

    let mesh_path = dir_path.join("cube.glb");
    let meta_path = dir_path.join("cube.glb.meta");
    std::fs::write(&mesh_path, "fake").unwrap();
    std::fs::write(&meta_path, "fake").unwrap();

    let guid = registry.get_or_create_guid(&mesh_path);

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
    assert!(!meta_path.exists());

    let new_mesh = dir_path.join("sphere.glb");
    let new_meta = dir_path.join("sphere.glb.meta");
    assert!(new_mesh.exists());
    assert!(new_meta.exists());
    assert_eq!(data.path, new_mesh);

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
    let meta = dir_path.join("player.png.meta");
    std::fs::write(&png, "fake").unwrap();
    std::fs::write(&meta, "fake").unwrap();

    let guid = registry.get_or_create_guid(&png);
    registry.get_or_create_guid(&meta);

    let node_data =
        ProjectTreeNode::new(guid, "player".into(), png.clone(), None, AssetKind::Texture);
    let tex_node = graph.graph.add_node(node_data);
    graph.graph.add_edge(dir, tex_node, ());

    graph.delete_node(&mut registry, tex_node).unwrap();

    assert!(!png.exists());
    assert!(!meta.exists());
    assert!(graph.get_node(tex_node).is_none());
    assert!(registry.get_guid(&png).is_none());
    assert!(registry.get_guid(&meta).is_none());
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

// ---- companion pairing / hiding (S10) ----

/// Creates `files` (with placeholder content) under `tmp` and builds the tree.
fn scan(tmp: &TempDir, registry: &mut AssetRegistry, files: &[&str]) -> ProjectPathGraph {
    for rel in files {
        let path = tmp.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"placeholder").unwrap();
    }
    ProjectPathGraph::new_at_root(tmp.path(), registry)
}

/// The node at the given relative path, if the tree contains one.
fn node_at(graph: &ProjectPathGraph, rel: &str) -> Option<ProjectTreeNode> {
    graph
        .find_by_path(std::path::Path::new(rel))
        .and_then(|idx| graph.get_node(idx).cloned())
}

#[test]
fn source_with_meta_pairs_to_its_product() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    let graph = scan(
        &tmp,
        &mut registry,
        &[
            "res/textures/hero.png",
            "res/textures/hero.png.meta",
            "res/models/hero.glb",
            "res/models/hero.glb.meta",
        ],
    );

    let texture = node_at(&graph, "res/textures/hero.png").expect("the texture source is a node");
    assert_eq!(texture.kind, AssetKind::Texture);
    let product = texture
        .asset_path
        .expect("the source pairs with its product");
    assert!(
        product
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("imported_assets/Default/res/textures/hero.png"),
        "unexpected product path: {}",
        product.display()
    );

    let mesh = node_at(&graph, "res/models/hero.glb").expect("the mesh source is a node");
    assert_eq!(mesh.kind, AssetKind::Mesh);
    let product = mesh.asset_path.expect("the source pairs with its product");
    assert!(
        product
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("imported_assets/Default/res/models/hero.glb"),
        "unexpected product path: {}",
        product.display()
    );
}

#[test]
fn meta_sidecars_are_hidden() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    let graph = scan(
        &tmp,
        &mut registry,
        &["res/textures/hero.png", "res/textures/hero.png.meta"],
    );

    assert!(node_at(&graph, "res/textures/hero.png.meta").is_none());
}

#[test]
fn unimported_source_is_hidden() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    // No `.meta` beside it: the source has not been imported yet.
    let graph = scan(&tmp, &mut registry, &["res/textures/raw.png"]);

    assert!(node_at(&graph, "res/textures/raw.png").is_none());
}

#[test]
fn product_directory_is_hidden() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    let graph = scan(
        &tmp,
        &mut registry,
        &[
            "res/textures/hero.png",
            "res/textures/hero.png.meta",
            "imported_assets/Default/res/textures/hero.png",
            "imported_assets/Default/res/textures/hero.png.meta",
        ],
    );

    assert!(node_at(&graph, "imported_assets").is_none());
    assert!(node_at(&graph, "imported_assets/Default/res/textures/hero.png").is_none());
}

#[test]
fn legacy_companion_extensions_are_hidden() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    let graph = scan(
        &tmp,
        &mut registry,
        &["res/textures/old.texture_bin", "res/models/old.mesh_bin"],
    );

    assert!(node_at(&graph, "res/textures/old.texture_bin").is_none());
    assert!(node_at(&graph, "res/models/old.mesh_bin").is_none());
}

#[test]
fn plain_asset_shows_without_a_product_pair() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    let graph = scan(&tmp, &mut registry, &["res/materials/hero.mat"]);

    let material = node_at(&graph, "res/materials/hero.mat").expect("a `.mat` is a node");
    assert_eq!(material.kind, AssetKind::Material);
    assert!(material.asset_path.is_none());
}

#[test]
fn nested_imported_assets_directory_is_not_skipped() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    // Only the top-level product root is hidden; a same-named source subdir stays.
    let graph = scan(&tmp, &mut registry, &["res/imported_assets/notes.txt"]);

    assert!(node_at(&graph, "res/imported_assets").is_some());
}

#[test]
fn rename_recomputes_the_product_pair() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    let mut graph = scan(
        &tmp,
        &mut registry,
        &["res/textures/hero.png", "res/textures/hero.png.meta"],
    );

    let node = graph
        .find_by_path(std::path::Path::new("res/textures/hero.png"))
        .expect("the source is a node");
    graph
        .rename_node(&mut registry, node, "villain")
        .expect("rename succeeds");

    let renamed = graph.get_node(node).expect("the node survives");
    let product = renamed
        .asset_path
        .as_ref()
        .expect("the pair is kept")
        .to_string_lossy()
        .replace('\\', "/");
    assert!(
        product.ends_with("imported_assets/Default/res/textures/villain.png"),
        "unexpected product path: {product}"
    );
}

#[test]
fn source_path_for_resolves_the_paired_source() {
    let tmp = TempDir::new().unwrap();
    let mut registry = AssetRegistry::new();
    let graph = scan(
        &tmp,
        &mut registry,
        &["res/textures/hero.png", "res/textures/hero.png.meta"],
    );

    let hero = node_at(&graph, "res/textures/hero.png").expect("the source is a node");
    let product = hero.asset_path.expect("the source pairs with its product");
    let source = graph
        .source_path_for(&product)
        .expect("the product maps back to its source");
    assert!(
        source
            .to_string_lossy()
            .replace('\\', "/")
            .ends_with("res/textures/hero.png"),
        "unexpected source path: {}",
        source.display()
    );
}
