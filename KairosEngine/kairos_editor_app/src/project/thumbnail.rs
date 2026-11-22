//! Decoded previews for the Project panel's texture rows.
//!
//! The asset browser draws a texture row from the texture itself (T16), so the
//! panel needs a small image in the row's 18 px icon box. Decoding belongs here,
//! on the host side, for three reasons: the UI thread must not read files, a 4K
//! texture must not be uploaded at full size to paint an icon, and a slow decode
//! must not stall a frame.
//!
//! The cache is lazy and asynchronous. HostView::project_thumbnail asks for one
//! path; the first call starts a worker thread that decodes and downscales the
//! image, and returns None (the row keeps its family icon). A later call uploads
//! the finished image as an egui texture and returns its id. Entries are keyed by
//! the absolute path; Thumbnails::clear drops everything when the watcher reports
//! an external change, so a replaced texture does not keep its old preview.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};

/// Longest edge of a decoded preview, in pixels.
///
/// Bigger than the 18 px icon box so a future grid view (M9) can reuse the same
/// cache without decoding again; the GPU cost of 32 x 32 is negligible.
const THUMBNAIL_PX: u32 = 32;
/// Files larger than this are not decoded: an asset browser should not pull a
/// 200 MB image into memory to paint an 18 px row icon.
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// The host's preview cache: one entry per absolute file path.
#[derive(Default)]
pub(crate) struct Thumbnails {
    entries: Mutex<HashMap<PathBuf, Entry>>,
}

enum Entry {
    /// A decode is running; the receiver yields the image, or None when the file
    /// is not an image, is too large or could not be read.
    Loading(Receiver<Option<ColorImage>>),
    /// Decoded, uploaded and ready to paint.
    Ready(TextureHandle),
    /// Decoding failed (or the file is not an image): do not retry every frame.
    Failed,
}

impl Thumbnails {
    /// Drop every cached preview.
    ///
    /// Called when the watcher reports that the project changed on disk: the
    /// image behind a path may have been replaced, and a preview is the one
    /// thing a stale cache shows the user. The next frame re-requests only the
    /// rows that are visible.
    pub(crate) fn clear(&self) {
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        entries.clear();
    }

    /// The preview of relative below root, starting its decode on first use.
    ///
    /// None means "not ready": the caller keeps its family icon and asks again
    /// next frame. The texture upload happens on the asking (UI) thread, which
    /// is the only thread that may touch the egui context.
    pub(crate) fn get(&self, ctx: &Context, root: &str, relative: &str) -> Option<TextureId> {
        let path = Path::new(root).join(relative);
        let mut entries = self
            .entries
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        if let Some(entry) = entries.get_mut(&path) {
            match entry {
                Entry::Ready(texture) => return Some(texture.id()),
                Entry::Failed => return None,
                Entry::Loading(result) => match result.try_recv() {
                    Ok(Some(image)) => {
                        let name = path.to_string_lossy().into_owned();
                        let texture = ctx.load_texture(name, image, TextureOptions::LINEAR);
                        let id = texture.id();
                        *entry = Entry::Ready(texture);
                        return Some(id);
                    }
                    Ok(None) => {
                        *entry = Entry::Failed;
                        return None;
                    }
                    Err(TryRecvError::Empty) => return None,
                    Err(TryRecvError::Disconnected) => {
                        *entry = Entry::Failed;
                        return None;
                    }
                },
            }
        }

        let (sender, receiver) = mpsc::channel();
        entries.insert(path.clone(), Entry::Loading(receiver));
        let worker_path = path.clone();
        let spawned = std::thread::Builder::new()
            .name(String::from("project-thumbnail"))
            .spawn(move || {
                // A dropped receiver means the cache was cleared while the
                // worker ran: the answer is no longer wanted.
                let _ = sender.send(decode(&worker_path));
            });
        if spawned.is_err() {
            entries.insert(path, Entry::Failed);
        }
        None
    }
}

/// Decode one file into a downscaled ColorImage, or None when it is not a
/// readable image or too large to be worth it.
fn decode(path: &Path) -> Option<ColorImage> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_FILE_BYTES {
        return None;
    }
    let image = image::open(path).ok()?;
    let thumbnail = image.thumbnail(THUMBNAIL_PX, THUMBNAIL_PX).to_rgba8();
    let size = [thumbnail.width() as usize, thumbnail.height() as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, thumbnail.as_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG fixture in the OS temp directory: width x height, solid red.
    fn png_fixture(width: u32, height: u32) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kairos-thumbnail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(format!("{width}x{height}.png"));
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 40, 40, 255]));
        image.save(&path).expect("write png");
        path
    }

    #[test]
    fn a_png_decodes_to_a_downscaled_square_bounded_image() {
        let path = png_fixture(100, 50);
        let image = decode(&path).expect("an image decodes");
        assert_eq!(
            image.size,
            [32, 16],
            "the longest edge is the thumbnail box"
        );
    }

    #[test]
    fn a_non_image_file_is_not_decoded() {
        let path =
            std::env::temp_dir().join(format!("kairos-thumbnail-text-{}.txt", std::process::id()));
        std::fs::write(&path, b"not an image").expect("write");
        assert!(decode(&path).is_none());
    }

    #[test]
    fn an_oversized_file_is_not_decoded() {
        let path =
            std::env::temp_dir().join(format!("kairos-thumbnail-big-{}.png", std::process::id()));
        let file = std::fs::File::create(&path).expect("create");
        file.set_len(MAX_FILE_BYTES + 1).expect("sparse file");
        assert!(
            decode(&path).is_none(),
            "the size guard runs before the decoder"
        );
    }

    #[test]
    fn the_cache_returns_the_preview_after_the_worker_finishes() {
        let path = png_fixture(64, 64);
        let root = path
            .parent()
            .expect("parent")
            .to_string_lossy()
            .into_owned();
        let relative = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        let ctx = Context::default();
        let thumbnails = Thumbnails::default();

        assert_eq!(
            thumbnails.get(&ctx, &root, &relative),
            None,
            "the first ask starts the decode and has nothing to paint yet"
        );

        let mut id = None;
        for _ in 0..200 {
            if let Some(found) = thumbnails.get(&ctx, &root, &relative) {
                id = Some(found);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let id = id.expect("the worker must deliver the preview");
        assert_eq!(
            thumbnails.get(&ctx, &root, &relative),
            Some(id),
            "a ready preview is cached, not decoded again"
        );

        thumbnails.clear();
        assert_eq!(
            thumbnails.get(&ctx, &root, &relative),
            None,
            "a cleared cache starts over"
        );
    }
}
