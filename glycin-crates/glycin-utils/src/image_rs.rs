use super::{Frame, ImageInfo, MemoryFormat, SharedMemory};

impl ImageInfo {
    pub fn from_decoder<'a, T: image::ImageDecoder<'a>>(
        decoder: &mut T,
        format_name: impl ToString,
    ) -> Self {
        let (width, height) = decoder.dimensions();

        Self::new(width, height, format_name.to_string())
    }
}

impl Frame {
    pub fn from_decoder<'a, T: image::ImageDecoder<'a>>(
        mut decoder: T,
    ) -> Result<Self, image::ImageError> {
        let color_type = decoder.color_type();

        let memory_format = MemoryFormat::from(color_type);
        let (width, height) = decoder.dimensions();
        let iccp = decoder.icc_profile().into();

        let mut memory = SharedMemory::new(decoder.total_bytes());
        decoder.read_image(&mut memory)?;
        let texture = memory.into_texture();

        let mut frame = Self::new(width, height, memory_format, texture);
        frame.iccp = iccp;

        Ok(frame)
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
