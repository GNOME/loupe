use crate::dbus::*;
use gio::prelude::*;
use glycin_utils::*;

pub type Result<T> = std::result::Result<T, Error>;

/// Image request builder
pub struct ImageRequest {
    file: gio::File,
    mime_type: Option<glib::GString>,
}

impl ImageRequest {
    pub fn new(file: gio::File) -> Self {
        Self {
            file,
            mime_type: None,
        }
    }

    pub async fn request<'a>(mut self) -> Result<Image<'a>> {
        let gfile_worker = GFileWorker::spawn(self.file.clone());
        let mime_type = Self::guess_mime_type(&gfile_worker).await.unwrap();

        let process = DecoderProcess::new(&mime_type).await;
        let info = process.init(gfile_worker).await?;

        self.mime_type = Some(mime_type);

        Ok(Image {
            process,
            info,
            request: self,
        })
    }

    async fn guess_mime_type(gfile_worker: &GFileWorker) -> Result<glib::GString> {
        let (content_type_data, unsure) =
            gio::content_type_guess(None::<String>, &gfile_worker.head().await);
        if unsure {
            if let Some(filename) = gfile_worker.file().basename() {
                return Ok(gio::content_type_guess(Some(filename), &[]).0);
            }
        }

        Ok(content_type_data)
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
