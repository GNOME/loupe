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
        let gfile_worker = GFileWorker::spawn(self.file.clone());
        let mime_type = Self::guess_mime_type(&gfile_worker).await.unwrap();
        dbg!(mime_type);

        let process = DecoderProcess::new().await;
        let info = process.init(gfile_worker).await?;

        Ok(Image {
            process,
            info,
            request: self,
        })
    }

    async fn guess_mime_type(gfile_worker: &GFileWorker) -> Result<glib::GString> {
        let (content_type_data, _) = gio::content_type_guess(None::<String>, &gfile_worker.head().await);

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
