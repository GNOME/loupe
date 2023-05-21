use glycin_utils::*;
use libheif_rs::{ColorProfile, ColorSpace, HeifContext, LibHeif, RgbChroma};
use std::io::Cursor;
use std::io::Read;
use std::sync::Mutex;
fn main() {
    Communication::spawn(ImgDecoder::default());
}

#[derive(Default)]
pub struct ImgDecoder {
    pub decoder: Mutex<Option<(HeifContext, Vec<u8>)>>,
}

impl Decoder for ImgDecoder {
    fn init(&self, mut stream: UnixStream, _mime_type: String) -> Result<ImageInfo, DecoderError> {
        let mut data = Vec::new();
        stream.read_to_end(&mut data).context_internal()?;

        let context = HeifContext::read_from_bytes(&data).context_failed()?;

        let handle = context.primary_image_handle().context_failed()?;

        let mut image_info =
            ImageInfo::new(handle.width(), handle.height(), "HEIF Container".into());
        image_info.exif = exif(&handle).into();

        // TODO: Later use libheif 1.16 to get info if there is a transformation
        image_info.transformations_applied = true;

        *self.decoder.lock().unwrap() = Some((context, data));
        Ok(image_info)
    }

    fn decode_frame(&self) -> Result<Frame, DecoderError> {
        let (context, data) =
            std::mem::take(&mut *self.decoder.lock().unwrap()).context_internal()?;
        decode(context, data)
    }
}

fn decode(context: HeifContext, _data: Vec<u8>) -> Result<Frame, DecoderError> {
    let handle = context.primary_image_handle().context_failed()?;

    let rgb_chroma = if handle.luma_bits_per_pixel() > 8 {
        if handle.has_alpha_channel() {
            #[cfg(target_endian = "little")]
            {
                RgbChroma::HdrRgbaLe
            }
            #[cfg(target_endian = "big")]
            {
                RgbChroma::HdrRgbaBe
            }
        } else {
            #[cfg(target_endian = "little")]
            {
                RgbChroma::HdrRgbLe
            }
            #[cfg(target_endian = "big")]
            {
                RgbChroma::HdrRgbBe
            }
        }
    } else if handle.has_alpha_channel() {
        RgbChroma::Rgba
    } else {
        RgbChroma::Rgb
    };

    let libheif = LibHeif::new();
    let image_result = libheif.decode(&handle, ColorSpace::Rgb(rgb_chroma), None);

    let mut image = match image_result {
        Err(err) if matches!(err.sub_code, libheif_rs::HeifErrorSubCode::UnsupportedCodec) => {
            return Err(DecoderError::UnsupportedImageFormat);
        }
        image => image.context_failed()?,
    };

    let icc_profile = if let Some(profile) = handle.color_profile_raw() {
        if [
            libheif_rs::color_profile_types::R_ICC,
            libheif_rs::color_profile_types::PROF,
        ]
        .contains(&profile.profile_type())
        {
            Some(profile.data)
        } else {
            None
        }
    } else {
        None
    };

    let plane = image.planes_mut().interleaved.context_failed()?;

    let memory_format = match rgb_chroma {
        RgbChroma::HdrRgbBe | RgbChroma::HdrRgbaBe | RgbChroma::HdrRgbLe | RgbChroma::HdrRgbaLe => {
            if let Ok(transmuted) = safe_transmute::transmute_many_pedantic_mut::<u16>(plane.data) {
                // Scale HDR pixels to 16bit (they are usually 10bit or 12bit)
                for pixel in transmuted.iter_mut() {
                    *pixel <<= 16 - plane.bits_per_pixel;
                }
            } else {
                eprintln!("Could not transform HDR (16bit) data to u16");
            }

            if handle.has_alpha_channel() {
                if handle.is_premultiplied_alpha() {
                    MemoryFormat::R16g16b16a16Premultiplied
                } else {
                    MemoryFormat::R16g16b16a16
                }
            } else {
                MemoryFormat::R16g16b16
            }
        }
        RgbChroma::Rgb | RgbChroma::Rgba => {
            if handle.has_alpha_channel() {
                if handle.is_premultiplied_alpha() {
                    MemoryFormat::R8g8b8a8Premultiplied
                } else {
                    MemoryFormat::R8g8b8a8
                }
            } else {
                MemoryFormat::R8g8b8
            }
        }
        RgbChroma::C444 => unreachable!(),
    };

    let mut memory = SharedMemory::new(plane.stride as u64 * plane.height as u64);
    Cursor::new(plane.data).read_exact(&mut memory).unwrap();
    let texture = memory.into_texture();

    let mut frame = Frame::new(plane.width, plane.height, memory_format, texture);
    frame.stride = plane.stride as u32;
    frame.iccp = icc_profile.into();

    Ok(frame)
}

fn exif(handle: &libheif_rs::ImageHandle) -> Option<Vec<u8>> {
    let mut meta_ids = vec![0];
    handle.metadata_block_ids(&mut meta_ids, b"Exif");

    if let Some(meta_id) = meta_ids.first() {
        match handle.metadata(*meta_id) {
            Ok(mut exif_bytes) => {
                if let Some(skip) = exif_bytes
                    .get(0..4)
                    .map(|x| u32::from_be_bytes(x.try_into().unwrap()) as usize)
                {
                    if exif_bytes.len() > skip + 4 {
                        exif_bytes.drain(0..skip + 4);
                        return Some(exif_bytes);
                    } else {
                        eprintln!("EXIF data has far too few bytes");
                    }
                } else {
                    eprintln!("EXIF data has far too few bytes");
                }
            }
            Err(_) => return None,
        }
    }

    None
}
