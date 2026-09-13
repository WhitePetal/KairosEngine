//! Tests for the IO layer: meta paths, in-memory and file readers, and the
//! source collections.

use std::{
    io::SeekFrom,
    path::{Path, PathBuf},
};

use futures_lite::{AsyncWriteExt, StreamExt, future::block_on};
use kairos_tasks::ConditionalSendFuture;

use crate::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, AssetSourceBuilders,
    AssetSourceId, AssetWatcher, AssetWriter, AssetWriterError, ErasedAssetReader,
    ErasedAssetWriter, Reader, VecReader, Writer, file::FileAssetReader, file::FileAssetWriter,
    file::resolve_base_path, get_meta_path, memory::MemoryAssetReader,
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

/// UFCS helpers for the write side. `FileAssetWriter` implements both
/// [`AssetWriter`] and its erased blanket impl, so the concrete trait is named
/// explicitly to avoid an ambiguous call.
fn write_bytes<W: AssetWriter>(
    writer: &W,
    path: &Path,
    bytes: &[u8],
) -> Result<(), AssetWriterError> {
    block_on(AssetWriter::write_bytes(writer, path, bytes))
}

fn write_meta_bytes<W: AssetWriter>(
    writer: &W,
    path: &Path,
    bytes: &[u8],
) -> Result<(), AssetWriterError> {
    block_on(AssetWriter::write_meta_bytes(writer, path, bytes))
}

/// Writes `bytes` through the byte-sink returned by [`AssetWriter::write`].
fn write_sink<W: AssetWriter>(
    writer: &W,
    path: &Path,
    bytes: &[u8],
) -> Result<(), AssetWriterError> {
    block_on(async {
        let mut sink = AssetWriter::write(writer, path).await?;
        futures_lite::AsyncWriteExt::write_all(&mut sink, bytes).await?;
        futures_lite::AsyncWriteExt::flush(&mut sink).await?;
        Ok(())
    })
}

/// Writes `bytes` through the meta byte-sink returned by
/// [`AssetWriter::write_meta`].
fn write_meta_sink<W: AssetWriter>(
    writer: &W,
    path: &Path,
    bytes: &[u8],
) -> Result<(), AssetWriterError> {
    block_on(async {
        let mut sink = AssetWriter::write_meta(writer, path).await?;
        AsyncWriteExt::write_all(&mut sink, bytes).await?;
        AsyncWriteExt::flush(&mut sink).await?;
        Ok(())
    })
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
fn memory_reader_over_an_empty_dir_misses_everything() {
    let reader = MemoryAssetReader::default();
    let error = read_all(&reader, Path::new("nope.png")).unwrap_err();
    assert_eq!(
        error,
        AssetReaderError::NotFound(PathBuf::from("nope.png"))
    );

    assert!(dir_entries(&reader, Path::new("")).unwrap().is_empty());
    assert!(!is_dir(&reader, Path::new("anything")).unwrap());
}

#[test]
fn base_path_prefers_the_kairos_asset_root_override() {
    // `KAIROS_ASSET_ROOT`, when set, wins over the working directory.
    let overridden = resolve_base_path(Some(std::ffi::OsString::from("some/root")));
    assert_eq!(overridden, PathBuf::from("some/root"));

    // With no override, the base path is the process working directory.
    let defaulted = resolve_base_path(None);
    assert_eq!(defaulted, std::env::current_dir().unwrap());
}

#[test]
fn builders_freeze_default_and_named_sources() {
    let mut builders = AssetSourceBuilders::default();
    builders.init_default_source("", None);
    builders.insert("named", AssetSourceBuilder::platform_default("res", None));
    let mut sources = builders.build_sources(false, false);

    let default = sources.get(AssetSourceId::Default).unwrap();
    assert_eq!(default.id(), AssetSourceId::Default);
    // `platform_default` always wires the unprocessed writer (the processor
    // writes source-side `.meta` through it), but only a processed writer —
    // which needs a processed path — makes `should_process` true.
    assert!(!default.should_process());
    assert!(default.writer().is_ok());

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

    // An absolute root replaces the cwd base rather than nesting under it. Use an
    // absolute path that is absolute on this platform: a Unix literal like
    // `/tmp` is only drive-relative on Windows, so `join` nests it under the cwd
    // instead of replacing it.
    let absolute = std::env::temp_dir();
    assert!(absolute.is_absolute(), "expected an absolute temp dir");
    let reader = FileAssetReader::new(&absolute);
    assert_eq!(reader.root_path(), absolute.as_path());
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
fn file_writer_round_trips_bytes_meta_rename_and_directories() {
    let dir = temp_dir("file_writer");

    // `create_root = true` creates the root eagerly; `false` leaves the tree
    // alone.
    let nested_root = dir.join("nested/root");
    let root = FileAssetWriter::new(&nested_root, true);
    assert_eq!(root.root_path(), nested_root.as_path());
    assert!(nested_root.is_dir());
    let untouched = dir.join("absent");
    let _ = FileAssetWriter::new(&untouched, false);
    assert!(!untouched.exists());

    let writer = FileAssetWriter::new(&dir, true);
    let reader = FileAssetReader::new(&dir);

    // The byte-sink form of `write`/`write_meta` creates missing parents.
    write_sink(&writer, Path::new("sink/asset.bin"), b"sink").unwrap();
    write_meta_sink(&writer, Path::new("sink/asset.bin"), b"sink-meta").unwrap();
    assert_eq!(
        read_all(&reader, Path::new("sink/asset.bin")).unwrap(),
        b"sink"
    );
    assert_eq!(
        read_meta_all(&reader, Path::new("sink/asset.bin")).unwrap(),
        b"sink-meta"
    );

    // The byte-slice form does too.
    write_bytes(&writer, Path::new("sub/hello.txt"), b"hello").unwrap();
    write_meta_bytes(&writer, Path::new("sub/hello.txt"), b"meta").unwrap();
    assert_eq!(
        read_all(&reader, Path::new("sub/hello.txt")).unwrap(),
        b"hello"
    );
    assert_eq!(
        read_meta_all(&reader, Path::new("sub/hello.txt")).unwrap(),
        b"meta"
    );

    // Renames move the asset and its sidecar, creating the destination parent.
    block_on(AssetWriter::rename(
        &writer,
        Path::new("sub/hello.txt"),
        Path::new("moved/hello.txt"),
    ))
    .unwrap();
    block_on(AssetWriter::rename_meta(
        &writer,
        Path::new("sub/hello.txt"),
        Path::new("moved/hello.txt"),
    ))
    .unwrap();
    assert!(read_all(&reader, Path::new("sub/hello.txt")).is_err());
    assert_eq!(
        read_all(&reader, Path::new("moved/hello.txt")).unwrap(),
        b"hello"
    );
    assert_eq!(
        read_meta_all(&reader, Path::new("moved/hello.txt")).unwrap(),
        b"meta"
    );

    // Removal takes the bytes and the sidecar.
    block_on(AssetWriter::remove(&writer, Path::new("moved/hello.txt"))).unwrap();
    block_on(AssetWriter::remove_meta(
        &writer,
        Path::new("moved/hello.txt"),
    ))
    .unwrap();
    assert!(read_all(&reader, Path::new("moved/hello.txt")).is_err());
    assert!(read_meta_all(&reader, Path::new("moved/hello.txt")).is_err());

    // Directories: create, refuse to remove a non-empty one, empty in place,
    // then remove outright.
    block_on(AssetWriter::create_directory(
        &writer,
        Path::new("tree/child"),
    ))
    .unwrap();
    assert!(is_dir(&reader, Path::new("tree/child")).unwrap());
    write_bytes(&writer, Path::new("tree/keep.txt"), b"x").unwrap();
    assert!(
        block_on(AssetWriter::remove_empty_directory(
            &writer,
            Path::new("tree")
        ))
        .is_err(),
        "remove_empty_directory refuses a non-empty directory"
    );

    block_on(AssetWriter::remove_assets_in_directory(
        &writer,
        Path::new("tree"),
    ))
    .unwrap();
    assert!(
        is_dir(&reader, Path::new("tree")).unwrap(),
        "the directory itself survives"
    );
    assert!(read_all(&reader, Path::new("tree/keep.txt")).is_err());
    assert!(is_dir(&reader, Path::new("tree/child")).is_err());

    write_bytes(&writer, Path::new("tree/again.txt"), b"y").unwrap();
    block_on(AssetWriter::remove_directory(&writer, Path::new("tree"))).unwrap();
    assert!(is_dir(&reader, Path::new("tree")).is_err());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_writer_create_root_failure_is_logged_not_fatal() {
    let dir = temp_dir("file_writer_create_root");
    // A plain file in the way makes `create_dir_all` fail.
    std::fs::write(dir.join("blocker"), b"x").unwrap();
    let blocked = dir.join("blocker/sub");

    // Bevy parity: a failed eager root creation is logged, not fatal; the
    // unwritable root surfaces on the first write instead.
    let writer = FileAssetWriter::new(&blocked, true);
    assert_eq!(writer.root_path(), blocked.as_path());
    assert!(
        write_bytes(&writer, Path::new("a.txt"), b"a").is_err(),
        "the unwritable root surfaces as a write error"
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
    let sources = builders.build_sources(false, false);

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
fn platform_default_wires_writers_and_should_process() {
    let dir = temp_dir("platform_default");
    let source_dir = dir.join("res");
    let processed_dir = dir.join("imported_assets/Default");
    std::fs::create_dir_all(&source_dir).unwrap();

    let mut builder = AssetSourceBuilder::platform_default(
        source_dir.to_str().unwrap(),
        Some(processed_dir.to_str().unwrap()),
    );
    let source = builder.build(AssetSourceId::Default, false, false);

    assert!(source.writer().is_ok());
    assert!(source.processed_reader().is_ok());
    assert!(source.processed_writer().is_ok());
    assert!(source.should_process());
    assert!(
        processed_dir.is_dir(),
        "the processed root is created eagerly so the first write has a home"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn should_process_follows_the_processed_writer() {
    // A processed reader alone does not make the source a processor source.
    let mut reader_only =
        AssetSourceBuilder::new(embedded_reader).with_processed_reader(embedded_reader);
    let source = reader_only.build(AssetSourceId::Default, false, false);
    assert!(source.processed_reader().is_ok());
    assert!(source.processed_writer().is_err());
    assert!(!source.should_process());

    // The processed writer is what marks a source as processed.
    let mut writer_backed = AssetSourceBuilder::new(embedded_reader).with_processed_writer(
        |_create_root| Some(Box::new(NoopWriter) as Box<dyn ErasedAssetWriter>),
    );
    let source = writer_backed.build(AssetSourceId::Default, false, false);
    assert!(source.processed_writer().is_ok());
    assert!(source.should_process());
}

#[test]
fn build_watch_creates_the_unprocessed_event_channel() {
    let mut builder = AssetSourceBuilder::new(embedded_reader)
        .with_watcher(|_sender| Some(Box::new(TestWatcher) as Box<dyn AssetWatcher>));

    let unwatched = builder.build(AssetSourceId::Default, false, false);
    assert!(unwatched.watcher().is_none());
    assert!(unwatched.event_receiver().is_none());

    let watched = builder.build(AssetSourceId::Default, true, false);
    assert!(watched.watcher().is_some());
    assert!(watched.event_receiver().is_some());
    assert!(watched.processed_watcher().is_none());
    assert!(watched.processed_event_receiver().is_none());
}

#[test]
fn build_watch_processed_creates_the_processed_event_channel() {
    let mut builder = AssetSourceBuilder::new(embedded_reader)
        .with_processed_watcher(|_sender| Some(Box::new(TestWatcher) as Box<dyn AssetWatcher>));

    let watched = builder.build(AssetSourceId::Default, false, true);
    assert!(watched.processed_watcher().is_some());
    assert!(watched.processed_event_receiver().is_some());
    assert!(watched.watcher().is_none());
    assert!(watched.event_receiver().is_none());
}

#[test]
fn build_sources_passes_watch_through_to_every_source() {
    let mut builders = AssetSourceBuilders::default();
    builders.init_default_source("res", None);
    builders.insert(
        "watched",
        AssetSourceBuilder::new(embedded_reader)
            .with_watcher(|_sender| Some(Box::new(TestWatcher) as Box<dyn AssetWatcher>)),
    );

    let sources = builders.build_sources(true, false);
    assert!(
        sources
            .get(AssetSourceId::Default)
            .unwrap()
            .event_receiver()
            .is_none()
    );
    assert!(
        sources
            .get("watched")
            .unwrap()
            .event_receiver()
            .is_some()
    );
}

fn embedded_reader() -> Box<dyn ErasedAssetReader> {
    Box::new(MemoryAssetReader::default())
}

/// A watcher that does nothing; tests only need one to exist so a watched source
/// can be built.
struct TestWatcher;

impl AssetWatcher for TestWatcher {}

/// A writer that panics if used; a source only needs a writer to exist to report
/// itself as processed.
struct NoopWriter;

impl AssetWriter for NoopWriter {
    fn write<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<Writer>, AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn write_meta<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<Writer>, AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn remove<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn remove_meta<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn rename<'a>(
        &'a self,
        _old_path: &'a Path,
        _new_path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn rename_meta<'a>(
        &'a self,
        _old_path: &'a Path,
        _new_path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn create_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn remove_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn remove_empty_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }

    fn remove_assets_in_directory<'a>(
        &'a self,
        _path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<(), AssetWriterError>> {
        async move { unimplemented!("NoopWriter") }
    }
}
