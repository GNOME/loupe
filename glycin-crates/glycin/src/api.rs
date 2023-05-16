use crate::dbus::*;
use glycin_utils::*;

pub type Result<T> = std::result::Result<T, Error>;

/// Image request builder
pub struct ImageRequest {
    file: gio::File,
}

impl ImageRequest {
    pub fn new(file: gio::File) -> Self {
        Self { file }
    }

    pub async fn request<'a>(self) -> Result<Image<'a>> {
        let process = DecoderProcess::new().await;
        let info = process
            .init(self.file.clone(), gio::Cancellable::new())
            .await?;

        Ok(Image {
            process,
            info,
            request: self,
        })
    }
}

/// Image handle containing metadata and allowing frame requests
pub struct Image<'a> {
    request: ImageRequest,
    process: DecoderProcess<'a>,
    info: ImageInfo,
}

impl<'a> Image<'a> {
    pub async fn next_frame(&self) -> Result<gdk::Texture> {
        self.process.decode_frame().await
    }

    pub fn info(&self) -> &ImageInfo {
        &self.info
    }

    pub fn request(&self) -> &ImageRequest {
        &self.request
    }
}
