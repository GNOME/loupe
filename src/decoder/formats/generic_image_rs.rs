use super::*;

use std::path::PathBuf;
use std::sync::RwLock;

#[derive(Debug)]
pub struct GenericImageRs {
    path: PathBuf,
    loaded: RwLock<bool>,
}

impl GenericImageRs {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            loaded: Default::default(),
        }
    }

    pub fn decode(
        &self,
        _decoding: Decoding,
        abstract_decoder: &Decoder,
    ) -> anyhow::Result<Option<DecodedImage>> {
        {
            let mut loaded = self.loaded.write().unwrap();
            if *loaded {
                return Ok(None);
            } else {
                *loaded = true;
            }
        }

        let dimensions = image::image_dimensions(&self.path)?;
        abstract_decoder.send_update(DecoderUpdate::Dimensions(dimensions));

        let dynamic_image = image::io::Reader::open(&self.path)?
            .with_guessed_format()?
            .decode()?;

        let decoded_image = DecodedImage::from(dynamic_image);

        Ok(Some(decoded_image))
    }
}
