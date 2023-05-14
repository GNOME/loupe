use glycin_utils::*;
use std::fs;
use std::io::Write;
use std::os::fd::IntoRawFd;
use std::pin::Pin;
use once_cell::sync::OnceCell;
use std::sync::Mutex;

fn main() {
    dbg!("Decoder started");
    async_std::task::block_on(decoder());
}

async fn decoder() {
    let communication = Communication::new(Box::new(ImgDecoder::default())).await;

    std::future::pending::<()>().await;
}

#[derive(Default)]
pub struct ImgDecoder {
    pub decoder: Mutex<Option<image::codecs::jpeg::JpegDecoder<fs::File>>>,
}

impl Decoder for ImgDecoder {
    fn init(&self, file: fs::File) -> Result<ImageInfo, String>
    {
        let decoder = image::codecs::jpeg::JpegDecoder::new(file).unwrap();
        *self.decoder.lock().unwrap() = Some(decoder);

        Ok(ImageInfo::default())
    }
    fn decode_frame(&self) -> Result<Frame, String> {
        let decoder = std::mem::take(&mut *self.decoder.lock().unwrap()).unwrap();
        let frame = Frame::from_decoder(decoder);
        Ok(frame)
    }
}

