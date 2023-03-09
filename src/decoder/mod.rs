///! Decodes several image formats
mod formats;
pub mod tiling;

use crate::image_metadata::ImageMetadata;
use formats::*;
use tiling::TilingStoreExt;

use anyhow::Context;
use arc_swap::ArcSwap;
use futures::channel::mpsc;
use gio::prelude::*;
use gtk::graphene;

use std::io::Read;
use std::sync::Arc;

pub use formats::RSVG_MAX_SIZE;

#[derive(Clone, Copy, Debug)]
pub enum ImageFormat {
    ImageRs(image_rs::ImageFormat),
    // TODO: Add details about contained format
    Heif,
    Svg,
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::ImageRs(format) => write!(f, "{:?}", format),
            Self::Heif => write!(f, "HEIF"),
            Self::Svg => write!(f, "SVG"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
/// Renderer requests new tiles
///
/// This happens for initial loading or when zoom level or viewing area changes.
pub struct TileRequest {
    pub viewport: graphene::Rect,
    pub zoom: f64,
}

#[derive(Debug, Clone)]
/// Signals update to the renderer
pub struct UpdateSender {
    sender: mpsc::UnboundedSender<DecoderUpdate>,
}

#[derive(Debug)]
/// Signals for renderer
pub enum DecoderUpdate {
    /// Dimensions of image in `TilingSore` available/updated
    Dimensions,
    /// Metadata available
    Metadata(ImageMetadata),
    /// Image format determined
    Format(ImageFormat),
    /// New image data available, redraw
    Redraw,
    /// And error occured during decoding
    Error(anyhow::Error),
}

impl UpdateSender {
    pub fn send(&self, update: DecoderUpdate) {
        let result = self.sender.unbounded_send(update);

        if let Err(err) = result {
            log::error!("Failed to send update: {err}");
        }
    }

    /// Send occuring errors to renderer
    pub fn spawn_error_handled<F>(&self, f: F) -> std::thread::JoinHandle<()>
    where
        F: FnOnce() -> Result<(), anyhow::Error> + Send + 'static,
    {
        let update_sender = self.clone();
        std::thread::spawn(move || {
            if let Err(err) = f() {
                update_sender.send(DecoderUpdate::Error(err));
            }
        })
    }
}

#[derive(Debug)]
pub struct Decoder {
    decoder: FormatDecoder,
}

#[derive(Debug)]
enum FormatDecoder {
    ImageRsOther(ImageRsOther),
    Svg(Svg),
    Heif(Heif),
}

impl Decoder {
    /// Get new image decoder
    ///
    /// The textures will be stored in the passed `TilingStore`.
    /// The renderer should listen to updates from the returned receiver.
    pub fn new(
        file: gio::File,
        tiles: Arc<ArcSwap<tiling::TilingStore>>,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<DecoderUpdate>)> {
        let path = file.path().context("Need a file path")?;
        let (sender, receiver) = mpsc::unbounded();

        let update_sender = UpdateSender { sender };
        tiles.set_update_sender(update_sender.clone());

        update_sender.send(DecoderUpdate::Metadata(ImageMetadata::load(&path)));

        let mut buf = Vec::new();
        let file = std::fs::File::open(&path)?;
        file.take(64).read_to_end(&mut buf)?;
        let format = image_rs::guess_format(&buf);

        let decoder = if let Ok(format) = format {
            match format {
                image_rs::ImageFormat::Avif => {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::Heif));
                    FormatDecoder::Heif(Heif::new(path, update_sender, tiles))
                }
                format => {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::ImageRs(format)));
                    FormatDecoder::ImageRsOther(ImageRsOther::new(path, update_sender, tiles))
                }
            }
        }
        // TODO: Use mime glib mime type detection
        else if [Some("svg"), Some("svgz")].contains(&path.extension().and_then(|x| x.to_str())) {
            update_sender.send(DecoderUpdate::Format(ImageFormat::Svg));
            FormatDecoder::Svg(Svg::new(path, update_sender, tiles))
        } else if [Some("heic"), Some("avif")].contains(&path.extension().and_then(|x| x.to_str()))
        {
            update_sender.send(DecoderUpdate::Format(ImageFormat::Heif));
            FormatDecoder::Heif(Heif::new(path, update_sender, tiles))
        } else {
            return None.context("unknown image format");
        };

        Ok((Self { decoder }, receiver))
    }

    /// Request missing tiles
    pub fn request(&self, tile_request: TileRequest) {
        match &self.decoder {
            FormatDecoder::Svg(svg) => svg.request(tile_request, self).unwrap(),
            FormatDecoder::Heif(_) => {}
            _ => {}
        };
    }
}
