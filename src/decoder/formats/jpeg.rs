use super::*;

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug)]
pub struct Jpeg {
    path: PathBuf,
    last_dimensions: RwLock<(u32, u32)>,
    original_dimensions: RwLock<Option<(u32, u32)>>,
}

impl Jpeg {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_dimensions: Default::default(),
            original_dimensions: Default::default(),
        }
    }

    pub fn decode(
        &self,
        decoding: Decoding,
        abstract_decoder: &Decoder,
    ) -> anyhow::Result<Option<DecodedImage>> {
        let requested_dimensions = (decoding.preload_width, decoding.preload_height);
        let mut last_dimensions = self.last_dimensions.write().unwrap();
        let mut original_dimensions = self.original_dimensions.write().unwrap();

        if *last_dimensions >= requested_dimensions {
            return Ok(None);
        }

        if let Some(original) = *original_dimensions {
            if *last_dimensions >= original {
                return Ok(None);
            }
        }

        let mut buf_reader = BufReader::new(File::open(&self.path)?);
        let mut decoder = jpeg_decoder::Decoder::new(&mut buf_reader);

        // read color profile and dimensions
        decoder.read_info()?;
        if let Some(info) = decoder.info() {
            let dimensions = (info.width.into(), info.height.into());
            abstract_decoder.send_update(DecoderUpdate::Dimensions(dimensions));

            if original_dimensions.is_none() {
                *original_dimensions = Some(dimensions);
            }
        } else {
            log::error!("JPEG info unexpectedly not available");
        }

        drop(original_dimensions);

        let icc_profile = decoder.icc_profile();

        buf_reader.rewind()?;

        let mut decoder = image::codecs::jpeg::JpegDecoder::new(&mut buf_reader)?;
        let new_dimensions = decoder.scale(
            decoding.preload_width as u16,
            decoding.preload_height as u16,
        );

        match new_dimensions {
            Ok((width, height)) => {
                *last_dimensions = (width as u32, height as u32);
            }
            Err(err) => {
                *last_dimensions = requested_dimensions;
                log::warn!("Failed to scale JPEG: {err}");
            }
        }

        // block new decodings until here because we now know the new size
        drop(last_dimensions);

        let dynamic_image = image::DynamicImage::from_decoder(decoder)?;
        let mut decoded_image = DecodedImage::from(dynamic_image);

        if let Some(icc_profile) = icc_profile {
            if let Err(err) = decoded_image.apply_icc_profile(&icc_profile) {
                log::warn!("Failed to apply color profile {:?}: {}", self.path, err);
            }
        }

        Ok(Some(decoded_image))
    }
}
