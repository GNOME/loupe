#[derive(Clone, Debug)]
pub struct ImageFormat {
    mime_type: glycin::MimeType,
    name: String,
}

impl ImageFormat {
    pub fn new(mime_type: glycin::MimeType, name: String) -> Self {
        Self { mime_type, name }
    }

    pub fn is_svg(&self) -> bool {
        matches!(
            self.mime_type.as_str(),
            "image/svg+xml" | "image/svg+xml-compressed"
        )
    }

    pub fn is_potentially_transparent(&self) -> bool {
        !matches!(self.mime_type.as_str(), "image/bmp" | "image/jpeg")
    }
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", self.name)
    }
}
