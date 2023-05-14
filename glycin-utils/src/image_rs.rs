use super::{Frame, ImageInfo, MemoryFormat, SharedMemory};

impl ImageInfo {
    pub fn from_decoder<'a, T: image::ImageDecoder<'a>>(decoder: &mut T) -> Self {
        let (width, height) = decoder.dimensions();

        Self {
            width,
            height,
            exif: None.into(),
            xmp: None.into(),
            transformations_applied: false,
        }
    }
}

impl Frame {
    pub fn from_decoder<'a, T: image::ImageDecoder<'a>>(mut decoder: T) -> Self {
        let color_type = decoder.color_type();

        let memory_format = MemoryFormat::from(color_type);
        let (width, height) = decoder.dimensions();
        let stride = (width * u32::from(color_type.bytes_per_pixel()))
            .try_into()
            .unwrap();
        let iccp = decoder.icc_profile().into();

        let mut memory = SharedMemory::new(decoder.total_bytes());
        decoder.read_image(&mut memory).unwrap();
        let texture = memory.into_texture();

        Self {
            width,
            height,
            memory_format,
            stride,
            texture,
            iccp,
            cicp: None.into(),
        }
    }
}

impl From<image::ColorType> for MemoryFormat {
    fn from(color_type: image::ColorType) -> Self {
        match color_type {
            image::ColorType::L8 => Self::L8,
            image::ColorType::La8 => Self::L8a8,
            image::ColorType::Rgb8 => Self::R8g8b8,
            image::ColorType::Rgba8 => Self::R8g8b8a8,
            image::ColorType::L16 => Self::L16,
            image::ColorType::La16 => Self::L16a16,
            image::ColorType::Rgb16 => Self::R16g16b16,
            image::ColorType::Rgba16 => Self::R16g16b16a16,
            image::ColorType::Rgb32F => Self::R32g32b32Float,
            image::ColorType::Rgba32F => Self::R32g32b32Float,
            _ => unimplemented!(),
        }
    }
}
