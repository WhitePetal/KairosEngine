use strum::{EnumCount, EnumIter};
use wgpu::TextureFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, EnumCount)]
#[repr(usize)]
pub enum InternalAttachmentId {
    FrameBufferColorAttachment,
    FrameBufferDepthStencilAttachment,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AttachmentLoadAction {
    /// Load the existing contents of the attachment.
    Load,
    /// Clear the attachment to a specified value.
    LoadClear,
    /// The contents of the attachment are undefined.
    DontCare,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AttachmentStoreAction {
    Store,
    /// The contents of the attachment are undefined after the pass.
    Discard,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentFormat {
    BGRA8Unorm,
    BGRA8UnormSrgb,
    #[default]
    RGBA8UNorm,
    RG11B10UFloat,
    D24S8,
}

#[derive(Debug, Default)]
pub struct Attachment {
    pub label: Option<&'static str>,
    pub width: u32,
    pub height: u32,
    pub format: AttachmentFormat,
    pub bind_internal_id: Option<InternalAttachmentId>,
}

impl Attachment {
    pub fn new(
        label: Option<&'static str>,
        width: u32,
        height: u32,
        format: AttachmentFormat,
    ) -> Self {
        Self {
            label,
            width,
            height,
            format,
            bind_internal_id: None,
        }
    }

    pub fn from_internal_id(bind_internal_id: InternalAttachmentId) -> Self {
        Self {
            label: None,
            width: 1,
            height: 1,
            format: AttachmentFormat::RGBA8UNorm,
            bind_internal_id: Some(bind_internal_id),
        }
    }
}

impl Into<TextureFormat> for AttachmentFormat {
    fn into(self) -> TextureFormat {
        match self {
            AttachmentFormat::BGRA8Unorm => TextureFormat::Bgra8Unorm,
            AttachmentFormat::BGRA8UnormSrgb => TextureFormat::Bgra8UnormSrgb,
            AttachmentFormat::RGBA8UNorm => TextureFormat::Rgba8Unorm,
            AttachmentFormat::RG11B10UFloat => TextureFormat::Rg11b10Ufloat,
            AttachmentFormat::D24S8 => TextureFormat::Depth24PlusStencil8,
        }
    }
}
