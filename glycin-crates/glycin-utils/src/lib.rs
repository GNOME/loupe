//! Utilities for building glycin decoders

#[cfg(feature = "image-rs")]
pub mod image_rs;

pub use anyhow;
pub use std::os::unix::net::UnixStream;

use anyhow::Context;
use gettextrs::gettext;
use serde::{Deserialize, Serialize};
use zbus::zvariant::{self, Optional, Type};

use std::ffi::CString;
use std::ops::{Deref, DerefMut};
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
use std::time::Duration;

#[derive(Debug)]
pub struct SharedMemory {
    memfd: RawFd,
    pub mmap: memmap::MmapMut,
}

impl SharedMemory {
    pub fn new(size: u64) -> Self {
        // TODO: use memfd crate again
        let memfd = nix::sys::memfd::memfd_create(
            &CString::new("glycin-frame").unwrap(),
            nix::sys::memfd::MemFdCreateFlag::MFD_CLOEXEC
                | nix::sys::memfd::MemFdCreateFlag::MFD_ALLOW_SEALING,
        )
        .expect("Failed to create memfd");
        nix::unistd::ftruncate(memfd, size.try_into().expect("Required memory too large"))
            .expect("Failed to set memfd size");
        let mmap = unsafe { memmap::MmapMut::map_mut(memfd) }.expect("Mailed to mmap memfd");

        Self { mmap, memfd }
    }

    pub fn into_texture(self) -> Texture {
        let owned_fd = unsafe { zvariant::OwnedFd::from_raw_fd(self.memfd) };
        Texture::MemFd(owned_fd)
    }
}

impl Deref for SharedMemory {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.mmap.deref()
    }
}

impl DerefMut for SharedMemory {
    fn deref_mut(&mut self) -> &mut [u8] {
        self.mmap.deref_mut()
    }
}

#[derive(Deserialize, Serialize, Type, Debug)]
pub struct DecodingRequest {
    pub fd: zvariant::OwnedFd,
    //pub mime_type: String,
}

/// Various image metadata
#[derive(Deserialize, Serialize, Type, Debug)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub exif: Optional<Vec<u8>>,
    pub xmp: Optional<Vec<u8>>,
    pub transformations_applied: bool,
}

impl ImageInfo {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            exif: None.into(),
            xmp: None.into(),
            transformations_applied: false,
        }
    }
}

#[derive(Deserialize, Serialize, Type, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub memory_format: MemoryFormat,
    pub texture: Texture,
    pub iccp: Optional<Vec<u8>>,
    pub cicp: Optional<Vec<u8>>,
    pub delay: Optional<Duration>,
}

#[derive(Deserialize, Serialize, Type, Debug)]
pub enum Texture {
    MemFd(zvariant::OwnedFd),
}

#[derive(Deserialize, Serialize, Type, Debug, Clone, Copy)]
pub enum MemoryFormat {
    B8g8r8a8Premultiplied,
    A8r8g8b8Premultiplied,
    R8g8b8a8Premultiplied,
    B8g8r8a8,
    A8r8g8b8,
    R8g8b8a8,
    A8b8g8r8,
    R8g8b8,
    B8g8r8,
    R16g16b16,
    R16g16b16a16Premultiplied,
    R16g16b16a16,
    R16g16b16Float,
    R16g16b16a16Float,
    R32g32b32Float,
    R32g32b32a32FloatPremultiplied,
    R32g32b32a32Float,
    L8,
    L8a8,
    L16,
    L16a16,
}

impl MemoryFormat {
    pub const fn n_bytes(self) -> MemoryFormatBytes {
        match self {
            MemoryFormat::L8 => MemoryFormatBytes::B1,
            MemoryFormat::L8a8 => MemoryFormatBytes::B2,
            MemoryFormat::L16 => MemoryFormatBytes::B2,
            MemoryFormat::L16a16 => MemoryFormatBytes::B4,
            MemoryFormat::B8g8r8a8Premultiplied => MemoryFormatBytes::B4,
            MemoryFormat::A8r8g8b8Premultiplied => MemoryFormatBytes::B4,
            MemoryFormat::R8g8b8a8Premultiplied => MemoryFormatBytes::B4,
            MemoryFormat::B8g8r8a8 => MemoryFormatBytes::B4,
            MemoryFormat::A8r8g8b8 => MemoryFormatBytes::B4,
            MemoryFormat::R8g8b8a8 => MemoryFormatBytes::B4,
            MemoryFormat::A8b8g8r8 => MemoryFormatBytes::B4,
            MemoryFormat::R8g8b8 => MemoryFormatBytes::B3,
            MemoryFormat::B8g8r8 => MemoryFormatBytes::B3,
            MemoryFormat::R16g16b16 => MemoryFormatBytes::B6,
            MemoryFormat::R16g16b16a16Premultiplied => MemoryFormatBytes::B8,
            MemoryFormat::R16g16b16a16 => MemoryFormatBytes::B8,
            MemoryFormat::R16g16b16Float => MemoryFormatBytes::B6,
            MemoryFormat::R16g16b16a16Float => MemoryFormatBytes::B8,
            MemoryFormat::R32g32b32Float => MemoryFormatBytes::B12,
            MemoryFormat::R32g32b32a32FloatPremultiplied => MemoryFormatBytes::B16,
            MemoryFormat::R32g32b32a32Float => MemoryFormatBytes::B16,
        }
    }
}

pub enum MemoryFormatBytes {
    B1 = 1,
    B2 = 2,
    B3 = 3,
    B4 = 4,
    B6 = 6,
    B8 = 8,
    B12 = 12,
    B16 = 16,
}

pub struct Communication {
    _dbus_connection: zbus::Connection,
}

impl Communication {
    pub async fn new(decoder: Box<dyn Decoder>) -> Self {
        let unix_stream = unsafe { UnixStream::from_raw_fd(3) };

        let instruction_handler = DecodingInstruction { decoder };
        let dbus_connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
            .p2p()
            .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
            .serve_at("/org/gnome/glycin", instruction_handler)
            .expect("Failed to setup instruction handler")
            .build()
            .await
            .expect("Failed to create private DBus connection");

        Communication {
            _dbus_connection: dbus_connection,
        }
    }
}

pub trait Decoder: Send + Sync {
    fn init(&self, stream: UnixStream) -> Result<ImageInfo, DecoderError>;
    fn decode_frame(&self) -> Result<Frame, DecoderError>;
}

struct DecodingInstruction {
    decoder: Box<dyn Decoder>,
}

#[zbus::dbus_interface(name = "org.gnome.glycin.DecodingInstruction")]
impl DecodingInstruction {
    async fn init(&self, message: DecodingRequest) -> Result<ImageInfo, Error> {
        let fd = message.fd.into_raw_fd();
        let stream = unsafe { UnixStream::from_raw_fd(fd) };

        let image_info = self.decoder.init(stream).unwrap();

        Ok(image_info)
    }

    async fn decode_frame(&self) -> Result<Frame, Error> {
        let frame = self.decoder.decode_frame().unwrap();
        Ok(frame)
    }
}

#[derive(zbus::DBusError, Debug)]
#[dbus_error(prefix = "org.gnome.glycin.Error")]
pub enum Error {
    #[dbus_error(zbus_error)]
    ZBus(zbus::Error),
    DecodingError(String),
    InternalDecoderError,
    UnsupportedImageFormat,
}

impl From<DecoderError> for Error {
    fn from(err: DecoderError) -> Self {
        match err {
            DecoderError::DecodingError(msg) => Self::DecodingError(msg),
            DecoderError::InternalDecoderError => Self::InternalDecoderError,
            DecoderError::UnsupportedImageFormat => Self::UnsupportedImageFormat,
        }
    }
}

#[derive(Debug)]
pub enum DecoderError {
    DecodingError(String),
    InternalDecoderError,
    UnsupportedImageFormat,
}

impl std::fmt::Display for DecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Self::DecodingError(err) => write!(f, "{err}"),
            Self::InternalDecoderError => {
                write!(f, "{}", gettext("Internal error while interpreting image"))
            }
            Self::UnsupportedImageFormat => write!(f, "{}", gettext("Unsupported image format")),
        }
    }
}

impl std::error::Error for DecoderError {}

impl From<anyhow::Error> for DecoderError {
    fn from(err: anyhow::Error) -> Self {
        eprintln!("Decoding error: {err:?}");
        Self::DecodingError(format!("{err}"))
    }
}

pub trait GenericContexts<T> {
    fn context_failed(self) -> anyhow::Result<T>;
    fn context_internal(self) -> Result<T, DecoderError>;
    fn context_unsupported(self) -> Result<T, DecoderError>;
}

impl<T, E> GenericContexts<T> for Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context_failed(self) -> anyhow::Result<T> {
        self.with_context(|| gettext("Failed to decode image"))
    }

    fn context_internal(self) -> Result<T, DecoderError> {
        self.map_err(|_| DecoderError::InternalDecoderError)
    }

    fn context_unsupported(self) -> Result<T, DecoderError> {
        self.map_err(|_| DecoderError::UnsupportedImageFormat)
    }
}

impl<T> GenericContexts<T> for Option<T> {
    fn context_failed(self) -> anyhow::Result<T> {
        self.with_context(|| gettext("Failed to decode image"))
    }

    fn context_internal(self) -> Result<T, DecoderError> {
        self.ok_or(DecoderError::InternalDecoderError)
    }

    fn context_unsupported(self) -> Result<T, DecoderError> {
        self.ok_or(DecoderError::UnsupportedImageFormat)
    }
}
