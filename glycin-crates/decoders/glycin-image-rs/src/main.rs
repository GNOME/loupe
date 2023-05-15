use glycin_utils::*;
use std::sync::Mutex;

fn main() {
    async_std::task::block_on(listener());
}

async fn listener() {
    let _connection = Communication::new(Box::<ImgDecoder>::default()).await;
    std::future::pending::<()>().await;
}

#[derive(Default)]
pub struct ImgDecoder {
    pub decoder: Mutex<Option<image::codecs::jpeg::JpegDecoder<UnixStream>>>,
}

impl Decoder for ImgDecoder {
    fn init(&self, stream: UnixStream) -> Result<ImageInfo, DecoderError> {
        let mut decoder = image::codecs::jpeg::JpegDecoder::new(stream).context_failed()?;
        let image_info = ImageInfo::from_decoder(&mut decoder);
        *self.decoder.lock().unwrap() = Some(decoder);
        Ok(image_info)
    }

    fn decode_frame(&self) -> Result<Frame, DecoderError> {
        let decoder = std::mem::take(&mut *self.decoder.lock().unwrap()).context_internal()?;
        let frame = Frame::from_decoder(decoder);
        Ok(frame)
    }
}
