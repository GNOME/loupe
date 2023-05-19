#![allow(clippy::large_enum_variant)]

use glycin_utils::*;
use image::codecs;

use std::sync::Mutex;

fn main() {
    Communication::spawn(ImgDecoder::default());
}

#[derive(Default)]
pub struct ImgDecoder {
    pub decoder: Mutex<Option<ImageRsDecoder<UnixStream>>>,
}

impl Decoder for ImgDecoder {
    fn init(&self, stream: UnixStream, mime_type: String) -> Result<ImageInfo, DecoderError> {
        let mut decoder = ImageRsDecoder::new(stream, &mime_type)?;
        let image_info = decoder.info();
        *self.decoder.lock().unwrap() = Some(decoder);
        Ok(image_info)
    }

    fn decode_frame(&self) -> Result<Frame, DecoderError> {
        let decoder = std::mem::take(&mut *self.decoder.lock().unwrap()).context_internal()?;
        let frame = decoder.frame().context_failed()?;
        Ok(frame)
    }
}

pub enum ImageRsDecoder<T: std::io::Read> {
    Jpeg(codecs::jpeg::JpegDecoder<T>),
    Png(codecs::png::PngDecoder<T>),
}

impl ImageRsDecoder<UnixStream> {
    fn new(stream: UnixStream, mime_type: &str) -> Result<Self, DecoderError> {
        Ok(match mime_type {
            "image/jpeg" => Self::Jpeg(codecs::jpeg::JpegDecoder::new(stream).context_failed()?),
            "image/png" => Self::Png(codecs::png::PngDecoder::new(stream).context_failed()?),
            _ => return Err(DecoderError::UnsupportedImageFormat),
        })
    }
}

impl<T: std::io::Read> ImageRsDecoder<T> {
    fn info(&mut self) -> ImageInfo {
        match self {
            Self::Jpeg(d) => ImageInfo::from_decoder(d, "JPEG"),
            Self::Png(d) => ImageInfo::from_decoder(d, "PNG"),
        }
    }

    fn frame(self) -> Result<Frame, image::ImageError> {
        match self {
            Self::Jpeg(d) => Frame::from_decoder(d),
            Self::Png(d) => Frame::from_decoder(d),
        }
    }
}
