// Copyright (c) 2023-2024 Sophie Herold
// Copyright (c) 2023 Julian Hofer
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Decodes several image formats

pub mod formats;
pub mod tiling;

use std::sync::Arc;

use anyhow::Result;
use async_channel as mpsc;
pub use formats::RSVG_MAX_SIZE;
use formats::*;

use self::tiling::SharedFrameBuffer;
use crate::deps::*;
use crate::metadata::Metadata;

#[derive(Clone, Copy, Debug)]
/// Renderer requests new tiles
///
/// This happens for initial loading or when zoom level or viewing area changes.
pub struct TileRequest {
    pub viewport: graphene::Rect,
    pub area: graphene::Rect,
    pub zoom: f64,
}

#[derive(Debug, Clone)]
/// Signals update to the renderer
pub struct UpdateSender {
    sender: mpsc::Sender<DecoderUpdate>,
}

#[derive(Debug)]
/// Signals for renderer (LpImage)
pub enum DecoderUpdate {
    /// Dimensions of image in `TilingSore` available/updated
    Dimensions,
    /// Metadata available
    Metadata(Box<Metadata>),
    /// New image data available, redraw
    Redraw,
    /// Start animation
    Animated,
    SpecificError(DecoderError),
    /// And error occurred during decoding
    GenericError(anyhow::Error),
}

#[derive(glib::Enum, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[enum_type(name = "LpDecoderError")]
//#[enum_dynamic(lazy_registration = true)]
#[repr(i32)]
pub enum DecoderError {
    /// Image format not supported or unknown
    UnsupportedFormat,
    /// No glycin-loaders installed
    NoLoadersConfigured,
    /// Memory limit exceeded
    OutOfMemory,
    #[default]
    None,
}

impl DecoderError {
    pub fn is_err(self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_ok(self) -> bool {
        !matches!(self, Self::None)
    }
}

impl glib::value::ToValueOptional for DecoderError {
    fn to_value_optional(s: Option<&Self>) -> glib::Value {
        s.unwrap_or(&Self::None).into()
    }
}

impl UpdateSender {
    pub fn send(&self, update: DecoderUpdate) {
        let result = self.sender.force_send(update);

        match result {
            Err(err) => log::error!("Failed to send update: {err}"),
            Ok(Some(msg)) => log::error!("Unexpectedly replaced the message {msg:?}"),
            _ => {}
        }
    }

    /// Send occurring errors to renderer
    pub fn spawn_error_handled<F>(&self, f: F) -> glib::JoinHandle<()>
    where
        F: std::future::Future<Output = Result<(), glycin::ErrorCtx>> + Send + 'static,
    {
        let update_sender = self.clone();
        glib::spawn_future(async move {
            let update_sender = update_sender.clone();

            let result: Result<(), glycin::ErrorCtx> = f.await;

            if let Err(err) = result {
                update_sender.send_error(err);
            }
        })
    }

    pub fn send_error(&self, err: glycin::ErrorCtx) {
        if err.unsupported_format().is_some() {
            self.send(DecoderUpdate::SpecificError(
                DecoderError::UnsupportedFormat,
            ));
        }
        if err.is_out_of_memory() {
            self.send(DecoderUpdate::SpecificError(DecoderError::OutOfMemory));
        } else if matches!(err.error(), glycin::Error::NoLoadersConfigured(_)) {
            self.send(DecoderUpdate::SpecificError(
                DecoderError::NoLoadersConfigured,
            ));
        }
        self.send(DecoderUpdate::GenericError(err.into()));
    }
}

#[derive(Debug)]
pub struct Decoder {
    decoder: FormatDecoder,
    update_sender: UpdateSender,
}

#[derive(Debug)]
enum FormatDecoder {
    Glycin(Glycin),
    Svg(Svg),
}

impl Decoder {
    /// Get new image decoder
    ///
    /// The textures will be stored in the passed `TiledImage`.
    /// The renderer should listen to updates from the returned receiver.
    pub async fn new(
        file: gio::File,
        mime_type: Option<String>,
        tiles: Arc<SharedFrameBuffer>,
    ) -> (Self, mpsc::Receiver<DecoderUpdate>) {
        let (sender, receiver) = mpsc::unbounded();

        let update_sender = UpdateSender { sender };
        tiles.set_update_sender(update_sender.clone());

        let format_decoder = Self::format_decoder(update_sender.clone(), file, mime_type, tiles);
        let decoder = Self {
            decoder: format_decoder,
            update_sender,
        };

        (decoder, receiver)
    }

    fn format_decoder(
        update_sender: UpdateSender,
        file: gio::File,
        mime_type: Option<String>,
        tiles: Arc<SharedFrameBuffer>,
    ) -> FormatDecoder {
        if let Some(mime_type) = mime_type {
            // Known things we want to match here are
            // - image/svg+xml
            // - image/svg+xml-compressed
            if mime_type.split('+').next() == Some("image/svg") {
                return FormatDecoder::Svg(Svg::new(file, update_sender, tiles));
            }
        }

        FormatDecoder::Glycin(Glycin::new(file, update_sender, tiles))
    }

    /// Request missing tiles
    pub fn request(&self, tile_request: TileRequest) {
        if let FormatDecoder::Svg(svg) = &self.decoder {
            if let Err(err) = svg.request(tile_request) {
                self.update_sender.send(DecoderUpdate::GenericError(err));
            }
        }
    }

    pub fn fill_frame_buffer(&self) {
        if let FormatDecoder::Glycin(decoder) = &self.decoder {
            decoder.fill_frame_buffer();
        } else {
            log::error!("Trying to fill frame buffer for decoder without animation support");
        }
    }
}
