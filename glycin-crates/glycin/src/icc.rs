use glycin_utils::{Frame, MemoryFormat, MemoryFormatBytes, Texture};
use rgb::AsPixels;
use safe_transmute::error::Error as TsmErr;

use std::os::fd::AsRawFd;

pub fn apply_transformation(frame: &Frame) -> anyhow::Result<()> {
    if let Some(iccp) = frame.iccp.as_ref() {
        let Texture::MemFd(fd) = &frame.texture;
        let raw_fd = fd.as_raw_fd();
        let mut mmap = unsafe { memmap::MmapMut::map_mut(raw_fd) }.unwrap();

        let memory_format = frame.memory_format;

        let res = match memory_format.n_bytes() {
            MemoryFormatBytes::B1 => transform::<u8>(iccp, memory_format, &mut mmap),
            MemoryFormatBytes::B2 => {
                let buf = safe_transmute::transmute_many_pedantic_mut(&mut mmap)
                    .map_err(TsmErr::without_src)?;
                transform::<u16>(iccp, memory_format, buf)
            }
            MemoryFormatBytes::B3 => {
                transform::<rgb::RGB<u8>>(iccp, memory_format, mmap.as_pixels_mut())
            }
            MemoryFormatBytes::B4 => {
                transform::<rgb::RGBA<u8>>(iccp, memory_format, mmap.as_pixels_mut())
            }
            MemoryFormatBytes::B6 => {
                let buf = safe_transmute::transmute_many_pedantic_mut(&mut mmap)
                    .map_err(TsmErr::without_src)?;
                transform::<rgb::RGB<u16>>(iccp, memory_format, buf.as_pixels_mut())
            }
            MemoryFormatBytes::B8 => {
                let buf = safe_transmute::transmute_many_pedantic_mut(&mut mmap)
                    .map_err(TsmErr::without_src)?;
                transform::<rgb::RGBA<u16>>(iccp, memory_format, buf.as_pixels_mut())
            }
            MemoryFormatBytes::B12 => {
                let buf = safe_transmute::transmute_many_pedantic_mut(&mut mmap)
                    .map_err(TsmErr::without_src)?;
                transform::<rgb::RGB<u32>>(iccp, memory_format, buf.as_pixels_mut())
            }
            MemoryFormatBytes::B16 => {
                let buf = safe_transmute::transmute_many_pedantic_mut(&mut mmap)
                    .map_err(TsmErr::without_src)?;
                transform::<rgb::RGBA<u32>>(iccp, memory_format, buf.as_pixels_mut())
            }
        };
        res.map_err(Into::into)
    } else {
        Ok(())
    }
}

fn transform<F: Copy>(
    icc_profile: &[u8],
    memory_format: MemoryFormat,
    buf: &mut [F],
) -> Result<(), lcms2::Error> {
    let icc_pixel_format = lcms_pixel_format(memory_format);
    let src_profile = lcms2::Profile::new_icc(icc_profile)?;
    let target_profile = lcms2::Profile::new_srgb();

    let transform = lcms2::Transform::new(
        &src_profile,
        icc_pixel_format,
        &target_profile,
        icc_pixel_format,
        lcms2::Intent::Perceptual,
    )?;

    transform.transform_in_place(buf);

    Ok(())
}

const fn lcms_pixel_format(format: MemoryFormat) -> lcms2::PixelFormat {
    match format {
        MemoryFormat::L8 => lcms2::PixelFormat::GRAY_8,
        MemoryFormat::L8a8 => lcms2::PixelFormat::GRAYA_8,
        MemoryFormat::L16 => lcms2::PixelFormat::GRAY_16,
        MemoryFormat::L16a16 => lcms2::PixelFormat::GRAYA_16,
        MemoryFormat::B8g8r8a8Premultiplied => premul(lcms2::PixelFormat::BGRA_8),
        MemoryFormat::A8r8g8b8Premultiplied => premul(lcms2::PixelFormat::ARGB_8),
        MemoryFormat::R8g8b8a8Premultiplied => premul(lcms2::PixelFormat::RGBA_8),
        MemoryFormat::B8g8r8a8 => lcms2::PixelFormat::BGRA_8,
        MemoryFormat::A8r8g8b8 => lcms2::PixelFormat::ARGB_8,
        MemoryFormat::R8g8b8a8 => lcms2::PixelFormat::RGBA_8,
        MemoryFormat::A8b8g8r8 => lcms2::PixelFormat::ABGR_8,
        MemoryFormat::R8g8b8 => lcms2::PixelFormat::RGB_8,
        MemoryFormat::B8g8r8 => lcms2::PixelFormat::BGR_8,
        MemoryFormat::R16g16b16 => lcms2::PixelFormat::RGB_16,
        MemoryFormat::R16g16b16a16Premultiplied => premul(lcms2::PixelFormat::RGBA_16),
        MemoryFormat::R16g16b16a16 => lcms2::PixelFormat::RGBA_16,
        MemoryFormat::R16g16b16Float => lcms2::PixelFormat::RGB_HALF_FLT,
        MemoryFormat::R16g16b16a16Float => lcms2::PixelFormat::RGBA_HALF_FLT,
        MemoryFormat::R32g32b32Float => lcms2::PixelFormat::RGB_FLT,
        MemoryFormat::R32g32b32a32FloatPremultiplied => premul(lcms2::PixelFormat::RGBA_FLT),
        MemoryFormat::R32g32b32a32Float => lcms2::PixelFormat::RGBA_FLT,
    }
}

const fn premul(format: lcms2::PixelFormat) -> lcms2::PixelFormat {
    let mut bytes = format.0;
    bytes |= 0b1 << 23;
    lcms2::PixelFormat(bytes)
}

#[test]
fn premul_test() {
    assert!(!lcms2::PixelFormat::RGBA_8.premultiplied());
    assert!(premul(lcms2::PixelFormat::RGBA_8).premultiplied());
}
