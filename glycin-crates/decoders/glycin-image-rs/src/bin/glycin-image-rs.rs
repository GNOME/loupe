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

pub enum ImageRsDecoder<T: std::io::Read> {
    Jpeg(codecs::jpeg::JpegDecoder<T>),
    Png(codecs::png::PngDecoder<T>),
    Gif(codecs::gif::GifDecoder<T>),
}

impl ImageRsDecoder<Reader> {
    fn new(data: Reader, mime_type: &str) -> Result<Self, DecoderError> {
        Ok(match mime_type {
            "image/jpeg" => Self::Jpeg(codecs::jpeg::JpegDecoder::new(data).context_failed()?),
            "image/png" => Self::Png(codecs::png::PngDecoder::new(data).context_failed()?),
            "image/gif" => Self::Gif(codecs::gif::GifDecoder::new(data).context_failed()?),
            _ => return Err(DecoderError::UnsupportedImageFormat),
        })
    }
}

impl<'a, T: std::io::Read + 'a> ImageRsDecoder<T> {
    fn info(&mut self) -> ImageInfo {
        match self {
            Self::Jpeg(d) => ImageInfo::from_decoder(d, "JPEG"),
            Self::Png(d) => ImageInfo::from_decoder(d, "PNG"),
            Self::Gif(d) => ImageInfo::from_decoder(d, "GIF"),
        }
    }

    fn frame(self) -> Result<Frame, image::ImageError> {
        match self {
            Self::Jpeg(d) => Frame::from_decoder(d),
            Self::Png(d) => Frame::from_decoder(d),
            Self::Gif(d) => Frame::from_decoder(d),
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
            Self::Png(d) => d.is_apng(),
            Self::Gif(_) => true,
            _ => false,
        }
    }
}
