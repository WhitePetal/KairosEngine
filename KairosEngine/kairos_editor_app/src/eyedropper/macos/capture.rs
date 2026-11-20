//! ScreenCaptureKit sampling for the macOS eyedropper.
//!
//! The picker keeps one display filter per screen. A capture asks for exactly
//! the 15x15 screen pixels around the pointer: sourceRect is in display
//! points, so its size is 15 / scale, while the output is 15x15 pixels.
//! Everything is drawn into an sRGB bitmap context before the bytes are read,
//! which converts P3 displays to the colour the editor means by "screen
//! colour" and gives us a known RGBA order.
//!
//! The picker asks for Screen Recording directly before requesting content;
//! the ScreenCaptureKit completion runs on a private queue, and the picker
//! polls the slot on the main thread.

use std::sync::{Arc, Mutex};

use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::AnyClass;
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextGetData, CGColorSpace, CGContext, CGDisplayBounds,
    CGImage, CGImageAlphaInfo, CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess,
    kCGColorSpaceSRGB,
};
#[cfg(test)]
use objc2_core_graphics::{CGBitmapContextCreateImage, CGBitmapContextCreateWithData};
use objc2_foundation::{NSArray, NSError};
use objc2_screen_capture_kit::{
    SCCaptureResolutionType, SCContentFilter, SCScreenshotManager, SCShareableContent,
    SCStreamConfiguration, SCWindow,
};

use crate::eyedropper::loupe::{GRID, HALF};

/// The pixels and colour of one completed capture.
#[derive(Clone)]
pub(super) struct CaptureResult {
    /// The global top-left point the capture was centred on.
    pub(super) point: (f64, f64),
    /// 15x15 RGBA pixels, row 0 = top.
    pub(super) block: [u8; GRID * GRID * 4],
    /// The centre pixel as sRGB bytes.
    pub(super) color: [u8; 4],
}

/// One display's filter and the geometry needed to aim a capture at it.
struct DisplayFilter {
    filter: Retained<SCContentFilter>,
    /// Full display bounds in global top-left points.
    bounds: CGRect,
    /// Backing pixels per point (1 on a non-Retina display, 2 on Retina).
    scale: f64,
}

/// A live sampling session. It is only touched on the main thread; the
/// completion blocks write through the shared state.
pub(super) struct CaptureSession {
    displays: Vec<DisplayFilter>,
    state: Arc<Mutex<super::overlay::OverlayState>>,
}

impl CaptureSession {
    /// Build the per-display filters. None when the system exposes no display
    /// we can capture (or the app cannot see them).
    pub(super) fn new(
        content: &SCShareableContent,
        state: Arc<Mutex<super::overlay::OverlayState>>,
    ) -> Option<Self> {
        let empty: Retained<NSArray<SCWindow>> = NSArray::new();
        let mut displays = Vec::new();
        // SAFETY: the content object is alive and the display array is read
        // only here.
        for display in unsafe { content.displays() }.iter() {
            // SAFETY: the display is one of the content's own objects.
            let id = unsafe { display.displayID() };
            let bounds = CGDisplayBounds(id);
            if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
                continue;
            }
            // SAFETY: the display outlives the filter (the filter retains the
            // content it needs), and the array is an empty window list.
            let filter = unsafe {
                SCContentFilter::initWithDisplay_excludingWindows(
                    SCContentFilter::alloc(),
                    &display,
                    &empty,
                )
            };
            // The filter is authoritative for how many display pixels a point
            // covers, including scaled Retina modes. This is not the same as
            // NSScreen.backingScaleFactor on every mode.
            let scale = (unsafe { filter.pointPixelScale() } as f64).clamp(1.0, 4.0);
            displays.push(DisplayFilter {
                filter,
                bounds,
                scale,
            });
        }
        if displays.is_empty() {
            return None;
        }
        Some(Self { displays, state })
    }

    /// Ask for the block under point (global top-left points).
    ///
    /// At most one capture is in flight. The completion writes the result into
    /// the shared state; super::ScreenPicker::poll drains it.
    pub(super) fn capture(&self, point: (f64, f64)) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.in_flight {
            return;
        }
        let Some(display) = self.displays.iter().find(|d| contains(d.bounds, point)) else {
            state.error = Some("no capturable display under the pointer".to_owned());
            return;
        };
        let rect = capture_rect(point, display.bounds, display.scale);
        // SAFETY: a fresh, default-configured ScreenCaptureKit object.
        let config = unsafe { SCStreamConfiguration::new() };
        // SAFETY: the configuration is a fresh object, and the values are in
        // the ranges the API documents (points for sourceRect, pixels for the
        // output size).
        unsafe {
            config.setWidth(GRID);
            config.setHeight(GRID);
            config.setShowsCursor(false);
            config.setSourceRect(rect);
            // The source rect is pixel-aligned and the output is its native
            // pixel size, so do not resample it into a different buffer size.
            config.setScalesToFit(false);
            config.setCaptureResolution(SCCaptureResolutionType::Best);
        }

        state.generation = state.generation.wrapping_add(1);
        let generation = state.generation;
        state.in_flight = true;
        state.pending = None;
        state.error = None;
        drop(state);

        let shared = Arc::clone(&self.state);
        // SAFETY: ScreenCaptureKit calls the block once with either a valid
        // image or an error. The image is read only inside the block, before
        // it returns, and copied into a plain byte array.
        let handler = RcBlock::new(move |image: *mut CGImage, error: *mut NSError| {
            let result = unsafe { image.as_ref() }.and_then(image_to_block);
            let message = if result.is_none() {
                Some(error_message(error))
            } else {
                None
            };
            let Ok(mut state) = shared.lock() else {
                return;
            };
            if state.generation != generation {
                return;
            }
            state.in_flight = false;
            if let Some((block, color)) = result {
                state.pending = Some(CaptureResult {
                    point,
                    block,
                    color,
                });
                state.last_capture = Some(point);
            } else if let Some(message) = message {
                state.error = Some(message);
            }
        });

        // SAFETY: the filter and the configuration stay alive for the call;
        // ScreenCaptureKit copies the block and invokes it once.
        unsafe {
            let _ = SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(
                &display.filter,
                &config,
                Some(&handler),
            );
        }
    }
}

/// Whether ScreenCaptureKit is available (macOS 14 and newer).
pub(super) fn available() -> bool {
    AnyClass::get(c"SCScreenshotManager").is_some()
        && AnyClass::get(c"SCShareableContent").is_some()
        && AnyClass::get(c"SCContentFilter").is_some()
}

/// Whether the process currently has Screen Recording permission.
pub(super) fn permission_granted() -> bool {
    CGPreflightScreenCaptureAccess()
}

/// Ask the system for Screen Recording permission. The alert appears from a
/// separate process, so this returns immediately.
pub(super) fn request_permission() -> bool {
    CGRequestScreenCaptureAccess()
}

/// The content slot: None means the request is still running.
pub(super) type ContentSlot = Arc<Mutex<Option<Result<Retained<SCShareableContent>, String>>>>;

/// Ask for the displays and windows. The picker requests Screen Recording
/// permission before this; the completion supplies the display list.
pub(super) fn request_content() -> ContentSlot {
    let slot: ContentSlot = Arc::new(Mutex::new(None));
    let state = Arc::clone(&slot);
    // SAFETY: the block is called once by ScreenCaptureKit with either a
    // valid object or an error; the object is retained before the block
    // returns because the system owns the original only for the call.
    let handler = RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let result = match unsafe { content.as_ref() } {
                Some(content) => match unsafe {
                    Retained::retain(
                        content as *const SCShareableContent as *mut SCShareableContent,
                    )
                } {
                    Some(content) => Ok(content),
                    None => {
                        Err("ScreenCaptureKit returned an unretainable content object".to_owned())
                    }
                },
                None => Err(error_message(error)),
            };
            if let Ok(mut slot) = state.lock() {
                *slot = Some(result);
            }
        },
    );
    // SAFETY: the completion block is copied by the API and the class was
    // checked by available.
    unsafe {
        SCShareableContent::getShareableContentExcludingDesktopWindows_onScreenWindowsOnly_completionHandler(
            false,
            true,
            &handler,
        );
    }
    slot
}

/// Draw the image into a 15x15 sRGB bitmap and return its RGBA bytes plus the
/// centre pixel. Drawing through a known colour space converts P3 captures to
/// sRGB, matching the Windows/X11 backends.
fn image_to_block(image: &CGImage) -> Option<([u8; GRID * GRID * 4], [u8; 4])> {
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    let width = GRID;
    let height = GRID;
    let bytes_per_row = width * 4;
    // SAFETY: a null data pointer asks Core Graphics to allocate the buffer;
    // the dimensions and the RGBA8 bitmap info are valid.
    let context = unsafe {
        CGBitmapContextCreate(
            std::ptr::null_mut(),
            width,
            height,
            8,
            bytes_per_row,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }?;
    // The context is a valid 15x15 bitmap context and the image is drawn
    // scaled into it, converting the display's colour space to sRGB.
    CGContext::clear_rect(
        Some(&context),
        CGRect::new(CGPoint::ZERO, CGSize::new(15.0, 15.0)),
    );
    CGContext::set_interpolation_quality(
        Some(&context),
        objc2_core_graphics::CGInterpolationQuality::None,
    );
    CGContext::draw_image(
        Some(&context),
        CGRect::new(CGPoint::ZERO, CGSize::new(15.0, 15.0)),
        Some(image),
    );
    // The context owns an allocated 15x15x4 buffer until it is dropped; the
    // copy happens before that.
    let data = CGBitmapContextGetData(Some(&context)) as *const u8;
    if data.is_null() {
        return None;
    }
    let mut block = [0u8; GRID * GRID * 4];
    // SAFETY: the buffer is at least width * height * 4 bytes.
    unsafe { std::ptr::copy_nonoverlapping(data, block.as_mut_ptr(), block.len()) };
    let center = (HALF * GRID + HALF) * 4;
    let color = [block[center], block[center + 1], block[center + 2], 255];
    Some((block, color))
}

/// A display-local capture rect in points. The rect is 15 device pixels, and
/// it is snapped to the pixel grid so the centre cell is the pixel under the
/// pointer rather than a fractional point between pixels.
fn capture_rect(point: (f64, f64), bounds: CGRect, scale: f64) -> CGRect {
    let local_x = point.0 - bounds.origin.x;
    let local_y = point.1 - bounds.origin.y;
    let cursor_pixel_x = (local_x * scale).floor();
    let cursor_pixel_y = (local_y * scale).floor();
    let origin_pixel_x = cursor_pixel_x - HALF as f64;
    let origin_pixel_y = cursor_pixel_y - HALF as f64;
    let span = GRID as f64 / scale;
    CGRect::new(
        CGPoint::new(origin_pixel_x / scale, origin_pixel_y / scale),
        CGSize::new(span, span),
    )
}

fn contains(bounds: CGRect, point: (f64, f64)) -> bool {
    point.0 >= bounds.origin.x
        && point.0 < bounds.origin.x + bounds.size.width
        && point.1 >= bounds.origin.y
        && point.1 < bounds.origin.y + bounds.size.height
}

fn error_message(error: *mut NSError) -> String {
    // SAFETY: the pointer is either null or a valid NSError for the duration
    // of the completion block.
    unsafe { error.as_ref() }
        .map(|error| error.localizedDescription().to_string())
        .unwrap_or_else(|| "ScreenCaptureKit returned no image".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> CGRect {
        CGRect::new(CGPoint::new(100.0, 50.0), CGSize::new(1440.0, 900.0))
    }

    #[test]
    fn capture_rect_is_centred_and_scales_by_backing() {
        let rect = capture_rect((820.0, 500.0), bounds(), 2.0);
        assert_eq!(rect.size.width, 7.5);
        assert_eq!(rect.size.height, 7.5);
        assert_eq!(rect.origin.x, 716.5);
        assert_eq!(rect.origin.y, 446.5);

        let rect = capture_rect((820.0, 500.0), bounds(), 1.0);
        assert_eq!(rect.size.width, 15.0);
        assert_eq!(rect.origin.x, 713.0);
        assert_eq!(rect.origin.y, 443.0);
    }

    #[test]
    fn display_bounds_contain_their_own_points() {
        assert!(contains(bounds(), (100.0, 50.0)));
        assert!(contains(bounds(), (1539.0, 949.0)));
        assert!(!contains(bounds(), (99.0, 50.0)));
        assert!(!contains(bounds(), (1540.0, 50.0)));
        assert!(!contains(bounds(), (100.0, 950.0)));
    }

    #[test]
    fn image_to_block_preserves_rows_and_srgb_bytes() {
        let mut bytes = [0u8; GRID * GRID * 4];
        // Top-left red, centre green.
        bytes[0] = 255;
        bytes[3] = 255;
        let centre = (HALF * GRID + HALF) * 4;
        bytes[centre + 1] = 255;
        bytes[centre + 3] = 255;

        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).unwrap();
        // SAFETY: the context points at a live 15x15 RGBA buffer; the buffer
        // outlives the context and the image built from it.
        let context = unsafe {
            CGBitmapContextCreateWithData(
                bytes.as_mut_ptr().cast(),
                GRID,
                GRID,
                8,
                GRID * 4,
                Some(&space),
                CGImageAlphaInfo::PremultipliedLast.0,
                None,
                std::ptr::null_mut(),
            )
        }
        .unwrap();
        let image = CGBitmapContextCreateImage(Some(&context)).unwrap();

        let (block, colour) = image_to_block(&image).unwrap();
        assert_eq!(&block[0..4], &[255, 0, 0, 255]);
        assert_eq!(&block[centre..centre + 4], &[0, 255, 0, 255]);
        assert_eq!(colour, [0, 255, 0, 255]);
    }

    #[test]
    fn a_negative_origin_display_is_matched_correctly() {
        let left = CGRect::new(CGPoint::new(-1920.0, 0.0), CGSize::new(1920.0, 1080.0));
        assert!(contains(left, (-1.0, 10.0)));
        assert!(!contains(left, (0.0, 10.0)));
    }
}
