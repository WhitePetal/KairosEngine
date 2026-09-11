//! Tests for the IO layer: meta paths, in-memory and file readers, and the
//! source collections.

use std::{
    io::SeekFrom,
    path::{Path, PathBuf},
};

use futures_lite::{StreamExt, future::block_on};

use crate::next::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceId, Reader, VecReader, embedded::EmbeddedAssetRegistry,
    embedded::EmbeddedAssetReader, file::FileAssetReader, get_meta_path,
};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "kairos_asset_next_io_{name}_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `AssetReader::read` and `ErasedAssetReader::read` both apply to concrete
/// readers, so tests disambiguate with UFCS through these helpers.
fn read_all<R: AssetReader>(reader: &R, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
    block_on(async {
        let mut handle = AssetReader::read(reader, path).await?;
        let mut buf = Vec::new();
        handle.read_to_end(&mut buf).await?;
        Ok(buf)
    })
}

fn read_meta_all<R: AssetReader>(reader: &R, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
    block_on(AssetReader::read_meta_bytes(reader, path))
}

fn dir_entries<R: AssetReader>(
    reader: &R,
    path: &Path,
) -> Result<Vec<PathBuf>, AssetReaderError> {
    block_on(async {
        let mut stream = AssetReader::read_directory(reader, path).await?;
        let mut entries = Vec::new();
        while let Some(entry) = stream.next().await {
            entries.push(entry);
        }
        Ok(entries)
    })
}

fn is_dir<R: AssetReader>(reader: &R, path: &Path) -> Result<bool, AssetReaderError> {
    block_on(AssetReader::is_directory(reader, path))
}

#[test]
fn get_meta_path_appends_the_suffix() {
    assert_eq!(get_meta_path(Path::new("foo")), PathBuf::from("foo.meta"));
    assert_eq!(
        get_meta_path(Path::new("foo.bar")),
        PathBuf::from("foo.bar.meta")
    );
    assert_eq!(
        get_meta_path(Path::new("a/b/c.ron")),
        PathBuf::from("a/b/c.ron.meta")
    );
}

#[test]
fn vec_reader_reads_and_seeks() {
    let mut reader = VecReader::new(b"abcdef".to_vec());

    let read = block_on(async {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();
        buf
    });
    assert_eq!(read, b"abcdef");

    block_on(async {
        futures_lite::AsyncSeekExt::seek(&mut reader, SeekFrom::Start(2))
            .await
            .unwrap();
    });

    let rest = block_on(async {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();
        buf
    });
    assert_eq!(rest, b"cdef");
}

#[test]
fn embedded_reader_misses_everything() {
    // The registry is empty by construction; its reader is the observable part.
    let _registry = EmbeddedAssetRegistry::new();

    let reader = EmbeddedAssetReader::new();
    let error = read_all(&reader, Path::new("nope.png")).unwrap_err();
    assert_eq!(
        error,
        AssetReaderError::NotFound(PathBuf::from("nope.png"))
    );

    assert!(dir_entries(&reader, Path::new("")).unwrap().is_empty());
    assert!(!is_dir(&reader, Path::new("anything")).unwrap());
}

#[test]
fn builders_freeze_default_and_named_sources() {
    let mut builders = AssetSourceBuilders::default();
    builders.init_default_source("", None);
    builders.insert("named", AssetSourceBuilder::platform_default("res", None));
    let mut sources = builders.build_sources();

    let default = sources.get(AssetSourceId::Default).unwrap();
    assert_eq!(default.id(), AssetSourceId::Default);
    assert!(!default.should_process());
    assert!(default.writer().is_err());

    let named = sources.get("named").unwrap();
    assert_eq!(named.id().as_str(), Some("named"));
    assert!(sources.get("missing").is_err());

    let ids: Vec<_> = sources.ids().collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(sources.iter().count(), 2);
    assert_eq!(sources.iter_processed().count(), 0);
    assert_eq!(sources.iter_mut().count(), 2);
}

#[test]
fn default_file_reader_is_rooted_at_the_working_directory() {
    let reader = FileAssetReader::new("");
    assert_eq!(reader.root_path(), std::env::current_dir().unwrap());

    // An absolute root replaces the cwd base rather than nesting under it.
    let reader = FileAssetReader::new("/tmp");
    assert_eq!(reader.root_path(), Path::new("/tmp"));
}

#[test]
fn file_reader_reads_bytes_meta_directories_and_misses() {
    let dir = temp_dir("file_reader");
    std::fs::write(dir.join("hello.txt"), b"hello").unwrap();
    std::fs::write(dir.join("hello.txt.meta"), b"meta").unwrap();
    std::fs::write(dir.join(".hidden"), b"hidden").unwrap();
    std::fs::create_dir(dir.join("sub")).unwrap();
    std::fs::write(dir.join("sub").join("inner.txt"), b"inner").unwrap();

    let reader = FileAssetReader::new(&dir);

    assert_eq!(read_all(&reader, Path::new("hello.txt")).unwrap(), b"hello");
    assert_eq!(
        read_meta_all(&reader, Path::new("hello.txt")).unwrap(),
        b"meta"
    );

    let missing = read_all(&reader, Path::new("nope.txt")).unwrap_err();
    assert_eq!(missing, AssetReaderError::NotFound(dir.join("nope.txt")));

    assert!(is_dir(&reader, Path::new("sub")).unwrap());
    assert!(!is_dir(&reader, Path::new("hello.txt")).unwrap());
    assert_eq!(
        is_dir(&reader, Path::new("absent")).unwrap_err(),
        AssetReaderError::NotFound(PathBuf::from("absent"))
    );

    let mut entries = dir_entries(&reader, Path::new("")).unwrap();
    entries.sort();
    assert_eq!(
        entries,
        vec![PathBuf::from("hello.txt"), PathBuf::from("sub")],
        "directory entries are source-relative; meta sidecars and hidden files are not listed"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn erased_reader_reads_through_a_source() {
    let dir = temp_dir("erased_reader");
    std::fs::write(dir.join("hello.txt"), b"erased").unwrap();

    let mut builders = AssetSourceBuilders::default();
    builders.init_default_source("", None);
    builders.insert(
        "named",
        AssetSourceBuilder::platform_default(dir.to_str().unwrap(), None),
    );
    let sources = builders.build_sources();

    let source: &AssetSource = sources.get("named").unwrap();
    let bytes = block_on(async {
        let mut handle = source.reader().read(Path::new("hello.txt")).await.unwrap();
        let mut buf = Vec::new();
        handle.read_to_end(&mut buf).await.unwrap();
        buf
    });
    assert_eq!(bytes, b"erased");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn platform_default_with_a_processed_root_marks_the_source_processed() {
    let mut builder = AssetSourceBuilder::platform_default("res", Some("imported"));
    let source = builder.build(AssetSourceId::Default);
    assert!(source.should_process());
    assert!(source.processed_reader().is_ok());
}
