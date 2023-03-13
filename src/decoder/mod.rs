///! Decodes several image formats
mod formats;
pub mod tiling;

pub use formats::ImageDimensionDetails;

use crate::deps::*;
use crate::i18n::*;
use crate::image_metadata::ImageMetadata;
use formats::*;
use tiling::TilingStoreExt;

use anyhow::{anyhow, bail, Context};
use arc_swap::ArcSwap;
use futures::channel::mpsc;
use gio::prelude::*;

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

pub use formats::{ImageFormat, RSVG_MAX_SIZE};

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
/// Signals for renderer (LpImage)
pub enum DecoderUpdate {
    /// Dimensions of image in `TilingSore` available/updated
    Dimensions(ImageDimensionDetails),
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
    pub async fn new(
        file: gio::File,
        tiles: Arc<ArcSwap<tiling::TilingStore>>,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<DecoderUpdate>)> {
        let path = file.path().context("Need a file path")?;
        let (sender, receiver) = mpsc::unbounded();

        let update_sender = UpdateSender { sender };
        tiles.set_update_sender(update_sender.clone());

        let decoder = gio::spawn_blocking(move || Self::format_decoder(update_sender, path, tiles))
            .await
            .map_err(|_| anyhow!("Constructing the FormatDecoder failed unexpectedly"))??;

        Ok((Self { decoder }, receiver))
    }

    fn format_decoder(
        update_sender: UpdateSender,
        path: PathBuf,
        tiles: Arc<ArcSwap<tiling::TilingStore>>,
    ) -> anyhow::Result<FormatDecoder> {
        update_sender.send(DecoderUpdate::Metadata(ImageMetadata::load(&path)));

        let mut buf = Vec::new();
        let file = std::fs::File::open(&path).context(i18n("Could not open image"))?;
        file.take(64)
            .read_to_end(&mut buf)
            .context(i18n("Could not read image"))?;

        // Try magic bytes first and than file name extension
        let format =
            image_rs::guess_format(&buf).or_else(|_| image_rs::ImageFormat::from_path(&path));

        if let Ok(format) = format {
            match format {
                image_rs::ImageFormat::Avif => {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::Heif));
                    return Ok(FormatDecoder::Heif(Heif::new(path, update_sender, tiles)));
                }
                format => {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::ImageRs(format)));
                    return Ok(FormatDecoder::ImageRsOther(ImageRsOther::new(
                        path,
                        format,
                        update_sender,
                        tiles,
                    )));
                }
            }
        } else {
            let file_info = gio::File::for_path(&path)
                .query_info(
                    gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                    gio::FileQueryInfoFlags::NONE,
                    gio::Cancellable::NONE,
                )
                .context("Could not read content type information")?;
            if let Some(content_type) = file_info
                .content_type()
                .as_ref()
                .and_then(|x| gio::content_type_get_mime_type(x))
            {
                // Known things we want to match here are
                // - image/svg+xml
                // - image/svg+xml-compressed
                if content_type.split('+').next() == Some("image/svg") {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::Svg));
                    return Ok(FormatDecoder::Svg(Svg::new(path, update_sender, tiles)));
                } else {
                    bail!(i18n_f("Unknown image format: {}", &[content_type.as_str()]));
                }
            }
        }

        bail!(i18n("Unknown image format"))
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
