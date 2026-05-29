pub enum InternalAttachmentId {
    FrameBuffer_ColorAttachment,
    FrameBuffer_DepthStencilAttachment,
    End,
}

pub enum AttachmentFormat {
    BGRA8Unorm,
    BGRA8UnormSrgb,
    RGBA8UNorm,
    RG11B10UFloat,
}

pub struct Attachment {
    pub width: u32,
    pub height: u32,
    pub format: AttachmentFormat,
    pub bind_internal_id: Option<InternalAttachmentId>,
}

impl Attachment {
    pub fn new(width: u32, height: u32, format: AttachmentFormat) -> Self {
        Self {
            width,
            height,
            format,
            bind_internal_id: None,
        }
    }

    pub fn from_internal_id(bind_internal_id: InternalAttachmentId) -> Self {
        Self {
            width: 1,
            height: 1,
            format: AttachmentFormat::RGBA8UNorm,
            bind_internal_id: Some(bind_internal_id),
        }
    }
}
