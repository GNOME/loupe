///! Decodes several image formats
pub mod formats;
pub mod tiling;

pub use formats::ImageDimensionDetails;

use crate::deps::*;
use crate::image_metadata::ImageMetadata;
use crate::util::gettext::*;
use crate::util::ToBufRead;
use formats::*;
use tiling::FrameBufferExt;

use anyhow::{anyhow, bail, Context, Result};
use arc_swap::ArcSwap;
use futures::channel::mpsc;
use gio::prelude::*;

use std::io::Read;
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
    update_sender: UpdateSender,
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
    /// The textures will be stored in the passed `TiledImage`.
    /// The renderer should listen to updates from the returned receiver.
    pub async fn new(
        file: gio::File,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<DecoderUpdate>)> {
        let (sender, receiver) = mpsc::unbounded();

        let update_sender = UpdateSender { sender };
        tiles.set_update_sender(update_sender.clone());

        let decoder = gio::spawn_blocking(
            glib::clone!(@strong update_sender => move || Self::format_decoder(
                update_sender,
                file,
                tiles
            )),
        )
        .await
        .map_err(|_| anyhow!("Constructing the FormatDecoder failed unexpectedly"))??;

        Ok((
            Self {
                decoder,
                update_sender,
            },
            receiver,
        ))
    }

    fn format_decoder(
        update_sender: UpdateSender,
        file: gio::File,
        tiles: Arc<ArcSwap<tiling::FrameBuffer>>,
    ) -> anyhow::Result<FormatDecoder> {
        update_sender.send(DecoderUpdate::Metadata(ImageMetadata::load(&file)));

        let format = Self::guess_format(&file)?;

        if let Some(format) = format {
            match format {
                image_rs::ImageFormat::Avif => {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::Heif));
                    return Ok(FormatDecoder::Heif(Heif::new(file, update_sender, tiles)));
                }
                format => {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::ImageRs(format)));
                    return Ok(FormatDecoder::ImageRsOther(ImageRsOther::new(
                        file,
                        format,
                        update_sender,
                        tiles,
                    )));
                }
            }
        } else {
            let file_info = file
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
                    return Ok(FormatDecoder::Svg(Svg::new(file, update_sender, tiles)));
                } else if ["image/avif", "image/heif", "image/heic"]
                    .contains(&content_type.as_str())
                {
                    update_sender.send(DecoderUpdate::Format(ImageFormat::Heif));
                    return Ok(FormatDecoder::Heif(Heif::new(file, update_sender, tiles)));
                } else {
                    bail!(gettext_f(
                        "Unknown image format: {}",
                        &[content_type.as_str()]
                    ));
                }
            }
        }

        bail!(gettext("Unknown image format"))
    }

    /// Request missing tiles
    pub fn request(&self, tile_request: TileRequest) {
        match &self.decoder {
            FormatDecoder::Svg(svg) => {
                if let Err(err) = svg.request(tile_request) {
                    self.update_sender.send(DecoderUpdate::Error(err));
                }
            }
            FormatDecoder::Heif(_) => {}
            _ => {}
        };
    }

    pub fn fill_frame_buffer(&self) {
        if let FormatDecoder::ImageRsOther(decoder) = &self.decoder {
            decoder.fill_frame_buffer();
        } else {
            log::error!("Trying to fill frame buffer for decoder without animation support");
        }
    }

    fn guess_format(file: &gio::File) -> anyhow::Result<Option<image_rs::ImageFormat>> {
        let mut buf = Vec::new();
        let fs_file = file
            .to_buf_read()
            .context(gettext("Could not open image"))?;
        fs_file
            .take(64)
            .read_to_end(&mut buf)
            .context(gettext("Could not read image"))?;

        // Try magic bytes first and than file name extension

        if let Ok(format) = image_rs::guess_format(&buf) {
            Ok(Some(format))
        } else if let Some(basename) = file.basename() {
            Ok(image_rs::ImageFormat::from_path(basename).ok())
        } else {
            Ok(None)
        }
    }
}
