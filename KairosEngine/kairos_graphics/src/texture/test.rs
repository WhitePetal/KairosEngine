use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use futures_lite::future::block_on;
use kairos_asset::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId};
use kairos_asset::{
    AssetEvent, AssetLoadFailedEvent, AssetProcessor, Assets, FileTransactionLogFactory,
    UntypedAssetLoadFailedEvent, handle_internal_asset_events,
};
use kairos_ecs::message::Messages;
use kairos_ecs::world::World;

use super::{Texture, TextureFormat, TextureLoader, TextureProcessor};

/// A source `Probe.png` runs through [`TextureProcessor`] and the resulting
/// product loads back through the processor's own server (layout ②, gated
/// reader).
#[test]
fn texture_processor_produces_the_product_and_it_loads() {
    let root = crate::test_support::file_dir("texture_produce");
    let unprocessed = root.join("res");
    let processed = root.join("imported_assets/Default");
    std::fs::create_dir_all(&unprocessed).unwrap();

    let source = unprocessed.join("Probe.png");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .save(&source)
        .expect("write the source png");

    let builder = AssetSourceBuilder::platform_default(
        unprocessed.to_str().unwrap(),
        Some(processed.to_str().unwrap()),
    );
    let mut builders = AssetSourceBuilders::default();
    builders.insert(AssetSourceId::Default, builder);
    let (processor, _sources) = AssetProcessor::new(&mut builders, false);

    processor
        .data()
        .set_log_factory(Box::new(FileTransactionLogFactory {
            file_path: root.join("log"),
        }))
        .expect("the log factory is set before the processor starts");
    processor.server().register_loader(TextureLoader);
    processor.register_processor(TextureProcessor);
    processor.set_default_processor::<TextureProcessor>("png");

    block_on(processor.run_initial_processing());

    // The product and its sidecar landed under the processed root, mirroring the
    // source's relative path.
    let product = processed.join("Probe.png");
    assert!(product.is_file(), "the product was not written");
    assert!(
        processed.join("Probe.png.meta").is_file(),
        "the product sidecar was not written"
    );

    // The product loads back through the processor's own server: it waits on the
    // gate, then decodes the product with `TextureLoader`'s settings.
    let server = processor.server().clone();
    let assets = Assets::<Texture>::default();
    server.register_asset(&assets);
    let mut world = World::new();
    world.insert_resource(assets);
    world.insert_resource(server.clone());
    world.insert_resource(Messages::<AssetEvent<Texture>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<Texture>>::default());
    world.insert_resource(Messages::<UntypedAssetLoadFailedEvent>::default());

    let handle = server.load::<Texture>("Probe.png");

    let mut loaded = None;
    for _ in 0..500 {
        handle_internal_asset_events(&mut world);
        if let Some(texture) = world.resource::<Assets<Texture>>().get(handle.id()) {
            loaded = Some((texture.width, texture.height, texture.format));
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }

    assert_eq!(loaded, Some((2, 2, TextureFormat::Rgba8Unorm)));
}

/// The workspace root holds `res/`; the crate lives one level below it.
fn res_textures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("kairos_graphics sits under the workspace root")
        .join("res/textures")
}

/// The committed `res/textures/white.png` source, with its `AssetAction::Process`
/// `.meta`, processes to a product that loads back through the gated server —
/// the texture half of the `res/` migration (issue #239).
#[test]
fn committed_white_texture_processes_and_loads() {
    let root = crate::test_support::file_dir("texture_committed_white");
    let unprocessed = root.join("res");
    let processed = root.join("imported_assets/Default");
    std::fs::create_dir_all(unprocessed.join("textures")).unwrap();

    // Stage the committed source and its sidecar so the `Process` meta drives the
    // run, not the `.png` default processor.
    std::fs::copy(
        res_textures_dir().join("white.png"),
        unprocessed.join("textures/white.png"),
    )
    .expect("copy white.png into the temp root");
    std::fs::copy(
        res_textures_dir().join("white.png.meta"),
        unprocessed.join("textures/white.png.meta"),
    )
    .expect("copy white.png.meta into the temp root");

    let builder = AssetSourceBuilder::platform_default(
        unprocessed.to_str().unwrap(),
        Some(processed.to_str().unwrap()),
    );
    let mut builders = AssetSourceBuilders::default();
    builders.insert(AssetSourceId::Default, builder);
    let (processor, _sources) = AssetProcessor::new(&mut builders, false);

    processor
        .data()
        .set_log_factory(Box::new(FileTransactionLogFactory {
            file_path: root.join("log"),
        }))
        .expect("the log factory is set before the processor starts");
    processor.server().register_loader(TextureLoader);
    processor.register_processor(TextureProcessor);
    processor.set_default_processor::<TextureProcessor>("png");

    block_on(processor.run_initial_processing());

    let product = processed.join("textures/white.png");
    assert!(product.is_file(), "the product was not written");
    assert!(
        processed.join("textures/white.png.meta").is_file(),
        "the product sidecar was not written"
    );

    let server = processor.server().clone();
    let assets = Assets::<Texture>::default();
    server.register_asset(&assets);
    let mut world = World::new();
    world.insert_resource(assets);
    world.insert_resource(server.clone());
    world.insert_resource(Messages::<AssetEvent<Texture>>::default());
    world.insert_resource(Messages::<AssetLoadFailedEvent<Texture>>::default());
    world.insert_resource(Messages::<UntypedAssetLoadFailedEvent>::default());

    let handle = server.load::<Texture>("textures/white.png");
    let mut loaded = None;
    for _ in 0..500 {
        handle_internal_asset_events(&mut world);
        if let Some(texture) = world.resource::<Assets<Texture>>().get(handle.id()) {
            loaded = Some((texture.width, texture.height, texture.format));
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }

    assert_eq!(loaded, Some((2, 2, TextureFormat::Rgba8Unorm)));
}

/// The processor honours explicit target dimensions in the settings.
#[test]
fn texture_processor_resizes_to_the_requested_dimensions() {
    let root = crate::test_support::file_dir("texture_resize");
    let unprocessed = root.join("res");
    std::fs::create_dir_all(&unprocessed).unwrap();

    let source = unprocessed.join("Probe.png");
    image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 255, 0, 255]))
        .save(&source)
        .expect("write the source png");

    let bytes = std::fs::read(&source).unwrap();
    let settings = super::TextureSettings {
        width: 2,
        height: 2,
        ..Default::default()
    };
    let (resolved, data) = super::TextureSettings::convert_source(&bytes, &settings)
        .expect("the source converts");
    assert_eq!((resolved.width, resolved.height), (2, 2));
    assert_eq!(data.len(), 1);
}
