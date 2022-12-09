mod decoded;
mod formats;

use crate::image_metadata::ImageMetadata;
use decoded::DecodedImage;
use formats::*;

use anyhow::Context;
use gio::prelude::*;
use gtk::gdk;
use tokio::sync::mpsc;

use std::io::Read;

#[derive(Clone, Copy, Debug)]
pub struct Decoding {
    preload_width: u32,
    preload_height: u32,
}

#[derive(Debug)]
pub enum DecoderUpdate {
    Dimensions((u32, u32)),
    Metadata(ImageMetadata),
    Format(String),
    Texture(gdk::Texture),
    Error(anyhow::Error),
}

impl Decoding {
    pub fn new(preload_width: u32, preload_height: u32) -> Self {
        Self {
            preload_width,
            preload_height,
        }
    }
}

type Updater = mpsc::Sender<DecoderUpdate>;

#[derive(Debug)]
pub struct Decoder {
    updater: Updater,
    decoder: FormatDecoder,
}

#[derive(Debug)]
enum FormatDecoder {
    Jpeg(Jpeg),
    GenericImageRs(GenericImageRs),
}

impl Decoder {
    pub fn new(file: gio::File) -> anyhow::Result<(Self, mpsc::Receiver<DecoderUpdate>)> {
        let path = file.path().context("Need a file path")?;
        let (updater, receiver) = mpsc::channel(1);

        updater.blocking_send(DecoderUpdate::Metadata(ImageMetadata::load(&file)));

        let mut buf = Vec::new();
        let file = std::fs::File::open(&path)?;
        file.take(64).read_to_end(&mut buf)?;
        let format = image::guess_format(&buf);

        let decoder = if let Ok(format) = format {
            match format {
                image::ImageFormat::Jpeg => FormatDecoder::Jpeg(Jpeg::new(path)),
                _ => FormatDecoder::GenericImageRs(GenericImageRs::new(path)),
            }
        } else {
            return None.context("unknown image format");
        };

        Ok((Self { updater, decoder }, receiver))
    }

    pub fn decode(&self, decoding: Decoding) {
        match self.texture(decoding) {
            Ok(Some(texture)) => self.send_update(DecoderUpdate::Texture(texture)),
            Ok(None) => log::debug!("No new texture needed"),
            Err(err) => {
                log::warn!("Error in decoding process: {err}");
                self.send_update(DecoderUpdate::Error(err))
            }
        };
    }

    pub fn send_update(&self, update: DecoderUpdate) {
        let result = self.updater.blocking_send(update);

        if let Err(err) = result {
            log::error!("Failed to send update: {err}");
        }
    }

    pub fn texture(&self, decoding: Decoding) -> anyhow::Result<Option<gdk::Texture>> {
        let decoded_image = match &self.decoder {
            FormatDecoder::Jpeg(jpeg) => jpeg.decode(decoding, &self)?,
            FormatDecoder::GenericImageRs(image) => image.decode(decoding, &self)?,
        };

        Ok(decoded_image.map(|img| img.into_texture()))
    }
}
