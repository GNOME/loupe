#![allow(clippy::large_enum_variant)]

use glycin_utils::*;
use image::codecs;
use image::AnimationDecoder;

use std::io::Cursor;
use std::io::Read;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

fn main() {
    Communication::spawn(ImgDecoder::default());
}

type Reader = Cursor<Vec<u8>>;

#[derive(Default)]
pub struct ImgDecoder {
    pub decoder: Mutex<Option<ImageRsDecoder<Reader>>>,
    pub thread: Mutex<Option<(std::thread::JoinHandle<()>, Receiver<Frame>)>>,
}

fn worker(decoder: ImageRsDecoder<Reader>, data: Reader, mime_type: String, send: Sender<Frame>) {
    let mut decoder = Some(decoder);

    std::thread::park();

    let mut nth_frame = 1;

    loop {
        if decoder.is_none() {
            decoder = ImageRsDecoder::new(data.clone(), &mime_type).ok();
        }
        let frames = std::mem::take(&mut decoder).unwrap().into_frames().unwrap();

        for frame in frames {
            match frame {
                Err(err) => {
                    eprintln!("Skipping frame: {err}");
                }
                Ok(frame) => {
                    nth_frame += 1;

                    let (delay_num, delay_den) = frame.delay().numer_denom_ms();

                    let delay = if delay_num == 0 || delay_den == 0 {
                        None
                    } else {
                        let micros = f64::round(delay_num as f64 * 1000. / delay_den as f64) as u64;
                        Some(std::time::Duration::from_micros(micros))
                    };

                    let buffer = frame.into_buffer();

                    let memory_format = MemoryFormat::R8g8b8a8;
                    let width = buffer.width();
                    let height = buffer.height();

                    let mut memory = SharedMemory::new(
                        width as u64 * height as u64 * memory_format.n_bytes() as u64,
                    );
                    Cursor::new(buffer.into_raw())
                        .read_exact(&mut memory)
                        .unwrap();
                    let texture = memory.into_texture();

                    let mut out_frame = Frame::new(width, height, memory_format, texture);
                    out_frame.delay = delay.into();

                    send.send(out_frame).unwrap();

                    // If not really an animation no need to keep the thread around
                    if delay.is_none() {
                        return;
                    }
                }
            }

            std::thread::park();
        }

        if nth_frame == 1 {
            panic!("No frames found");
        }
    }
}

impl Decoder for ImgDecoder {
    fn init(&self, mut stream: UnixStream, mime_type: String) -> Result<ImageInfo, DecoderError> {
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).context_internal()?;
        let data = Cursor::new(buf);

        let mut decoder = ImageRsDecoder::new(data.clone(), &mime_type)?;
        let image_info = decoder.info();

        if decoder.is_animated() {
            let (send, recv) = channel();
            let thead = std::thread::spawn(move || worker(decoder, data, mime_type, send));
            *self.thread.lock().unwrap() = Some((thead, recv));
        } else {
            *self.decoder.lock().unwrap() = Some(decoder);
        }

        Ok(image_info)
    }

    fn decode_frame(&self) -> Result<Frame, DecoderError> {
        let frame = if let Some(decoder) = std::mem::take(&mut *self.decoder.lock().unwrap()) {
            decoder.frame().context_failed()?
        } else if let Some((ref thread, ref recv)) = *self.thread.lock().unwrap() {
            thread.thread().unpark();
            recv.recv().unwrap()
        } else {
            return Err(DecoderError::InternalDecoderError);
        };

        Ok(frame)
    }
}

pub enum ImageRsDecoder<T: std::io::Read + std::io::Seek> {
    Bmp(codecs::bmp::BmpDecoder<T>),
    Dds(codecs::dds::DdsDecoder<T>),
    Farbfeld(codecs::farbfeld::FarbfeldDecoder<T>),
    Gif(codecs::gif::GifDecoder<T>),
    //Hdr(codecs::hdr::HdrDecoder<T>),
    Ico(codecs::ico::IcoDecoder<T>),
    Jpeg(codecs::jpeg::JpegDecoder<T>),
    OpenExr(codecs::openexr::OpenExrDecoder<T>),
    Png(codecs::png::PngDecoder<T>),
    Pnm(codecs::pnm::PnmDecoder<T>),
    Qoi(codecs::qoi::QoiDecoder<T>),
    Tga(codecs::tga::TgaDecoder<T>),
    Tiff(codecs::tiff::TiffDecoder<T>),
    WebP(codecs::webp::WebPDecoder<T>),
}

impl ImageRsDecoder<Reader> {
    fn new(data: Reader, mime_type: &str) -> Result<Self, DecoderError> {
        Ok(match mime_type {
            "image/bmp" => Self::Bmp(codecs::bmp::BmpDecoder::new(data).context_failed()?),
            "image/vnd-ms.dds" => Self::Dds(codecs::dds::DdsDecoder::new(data).context_failed()?),
            "image/x-ff" => {
                Self::Farbfeld(codecs::farbfeld::FarbfeldDecoder::new(data).context_failed()?)
            }
            "image/gif" => Self::Gif(codecs::gif::GifDecoder::new(data).context_failed()?),
            //"image/vnd.radiance" => Self::Hdr(codecs::hdr::HdrDecoder::new(data).context_failed()?),
            "image/x-icon" => Self::Ico(codecs::ico::IcoDecoder::new(data).context_failed()?),
            "image/jpeg" => Self::Jpeg(codecs::jpeg::JpegDecoder::new(data).context_failed()?),
            "image/x-exr" => {
                Self::OpenExr(codecs::openexr::OpenExrDecoder::new(data).context_failed()?)
            }
            "image/png" => Self::Png(codecs::png::PngDecoder::new(data).context_failed()?),
            "image/x-portable-bitmap"
            | "image/x-portable-graymap"
            | "image/x-portable-pixmap"
            | "image/x-portable-anymap" => {
                Self::Pnm(codecs::pnm::PnmDecoder::new(data).context_failed()?)
            }
            "image/x-qoi" => Self::Qoi(codecs::qoi::QoiDecoder::new(data).context_failed()?),
            "image/x-targa" | "image/x-tga" => {
                Self::Tga(codecs::tga::TgaDecoder::new(data).context_failed()?)
            }
            "image/tiff" => Self::Tiff(codecs::tiff::TiffDecoder::new(data).context_failed()?),
            "image/webp" => Self::WebP(codecs::webp::WebPDecoder::new(data).context_failed()?),

            _ => return Err(DecoderError::UnsupportedImageFormat),
        })
    }
}

impl<'a, T: std::io::Read + std::io::Seek + 'a> ImageRsDecoder<T> {
    fn info(&mut self) -> ImageInfo {
        match self {
            Self::Bmp(d) => ImageInfo::from_decoder(d, "BMP"),
            Self::Dds(d) => ImageInfo::from_decoder(d, "DDS"),
            Self::Farbfeld(d) => ImageInfo::from_decoder(d, "Farbfeld"),
            Self::Gif(d) => ImageInfo::from_decoder(d, "GIF"),
            //Self::Hdr(d) => ImageInfo::from_decoder(d, "Radiance HDR"),
            Self::Ico(d) => ImageInfo::from_decoder(d, "ICO"),
            Self::Jpeg(d) => ImageInfo::from_decoder(d, "JPEG"),
            Self::OpenExr(d) => ImageInfo::from_decoder(d, "OpenEXR"),
            Self::Png(d) => ImageInfo::from_decoder(d, "PNG"),
            Self::Pnm(d) => ImageInfo::from_decoder(d, "PNM"),
            Self::Qoi(d) => ImageInfo::from_decoder(d, "QOI"),
            Self::Tga(d) => ImageInfo::from_decoder(d, "TGA"),
            Self::Tiff(d) => ImageInfo::from_decoder(d, "TIFF"),
            Self::WebP(d) => ImageInfo::from_decoder(d, "WebP"),
        }
    }

    fn frame(self) -> Result<Frame, image::ImageError> {
        match self {
            Self::Bmp(d) => Frame::from_decoder(d),
            Self::Dds(d) => Frame::from_decoder(d),
            Self::Farbfeld(d) => Frame::from_decoder(d),
            Self::Gif(d) => Frame::from_decoder(d),
            //Self::Hdr(d) => Frame::from_decoder(d),
            Self::Ico(d) => Frame::from_decoder(d),
            Self::Jpeg(d) => Frame::from_decoder(d),
            Self::OpenExr(d) => Frame::from_decoder(d),
            Self::Png(d) => Frame::from_decoder(d),
            Self::Pnm(d) => Frame::from_decoder(d),
            Self::Qoi(d) => Frame::from_decoder(d),
            Self::Tga(d) => Frame::from_decoder(d),
            Self::Tiff(d) => Frame::from_decoder(d),
            Self::WebP(d) => Frame::from_decoder(d),
        }
    }

    fn into_frames(self) -> Option<image::Frames<'a>> {
        match self {
            Self::Png(d) => Some(d.apng().into_frames()),
            Self::Gif(d) => Some(d.into_frames()),
            _ => None,
        }
    }

    fn is_animated(&self) -> bool {
        match self {
            Self::Gif(_) => true,
            Self::Png(d) => d.is_apng(),
            Self::WebP(d) => d.has_animation(),
            _ => false,
        }
    }
}
