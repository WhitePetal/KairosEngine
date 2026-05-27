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
}

impl Attachment {
    pub fn new(width: u32, height: u32, format: AttachmentFormat) -> Self {
        Self {
            width,
            height,
            format,
        }
    }
}
