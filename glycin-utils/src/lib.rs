pub mod image_rs;

use serde::{Deserialize, Serialize};
//use std::num::NonZeroU32;
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
//use std::time::Duration;
use std::ffi::CString;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::os::fd::{AsRawFd, RawFd};
use zbus::zvariant::{self, Optional, Type};

#[derive(Debug)]
pub struct SharedMemory {
    memfd: RawFd,
    pub mmap: memmap::MmapMut,
}

impl SharedMemory {
    pub fn new(size: u64) -> Self {
        let memfd = nix::sys::memfd::memfd_create(
            &CString::new("glycin-frame").unwrap(),
            nix::sys::memfd::MemFdCreateFlag::MFD_CLOEXEC
                | nix::sys::memfd::MemFdCreateFlag::MFD_ALLOW_SEALING,
        )
        .expect("Failed to create memfd");
        nix::unistd::ftruncate(memfd, size.try_into().expect("Required memory too large"))
            .expect("Failed to set memfd size");
        let mmap = unsafe { memmap::MmapMut::map_mut(&memfd) }.expect("Mailed to mmap memfd");

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
    pub fd: zvariant::Fd,
    //pub mime_type: String,
}

#[derive(Deserialize, Serialize, Type, Debug, Default)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub exif: Optional<Vec<u8>>,
    pub xmp: Optional<Vec<u8>>,
    pub transformations_applied: bool,
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
    //pub delay: Optional<Duration>,
}

#[derive(Deserialize, Serialize, Type, Debug)]
pub enum Texture {
    MemFd(zvariant::OwnedFd),
}

#[derive(Deserialize, Serialize, Type, Debug)]
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

pub struct Communication<'a> {
    _dbus_connection: zbus::Connection,
    decoding_update: DecodingUpdateProxy<'a>,
}

pub type DecodeFrame<T> = Box<dyn Fn(T) -> Result<Frame, Error> + Send + Sync >;
pub type DecoderInit = Box<dyn Fn(fs::File) -> Result<Frame, Error> + Send + Sync >;

impl<'a> Communication<'a> {
    pub async fn new(decoder: Box<dyn Decoder>) -> Communication<'a> {
        let unix_stream = unsafe { UnixStream::from_raw_fd(3) };

        let instruction_handler = DecodingInstruction {decoder};
        let dbus_connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
            .p2p()
            .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
            .serve_at("/org/gnome/glycin", instruction_handler).expect("Failed to setup instruction handler")
            .build()
            .await
            .expect("Failed to create private DBus connection");

        let decoding_update = DecodingUpdateProxy::new(&dbus_connection)
            .await
            .expect("Failed to create decoding update proxy");

        Communication {
            _dbus_connection: dbus_connection,
            decoding_update,
        }
    }

    pub async fn send_image_info(&self, message: ImageInfo) {
        self.decoding_update
            .send_image_info(message)
            .await
            .expect("Failed to send image info");
    }
    pub async fn send_frame(&self, message: Frame) {
        self.decoding_update
            .send_frame(message)
            .await
            .expect("Failed to send image frame");
    }
}

#[zbus::dbus_proxy(
    interface = "org.gnome.glycin.DecodingUpdate",
    default_path = "/org/gnome/glycin"
)]
trait DecodingUpdate {
    async fn send_image_info(&self, message: ImageInfo) -> zbus::Result<()>;
    async fn send_frame(&self, message: Frame) -> zbus::Result<()>;
}

pub trait Decoder:  Send + Sync {
    fn init(&self, file: fs::File) -> Result<ImageInfo, String>;
    fn decode_frame(&self) -> Result<Frame, String>;
}

struct DecodingInstruction {
    decoder: Box<dyn Decoder>,
}

#[zbus::dbus_interface(name = "org.gnome.glycin.DecodingInstruction")]
impl DecodingInstruction {
    async fn decoding_request(&self, message: DecodingRequest
    ) -> Result<Frame, Error> {
        dbg!(&message);
        let fd = message.fd.as_raw_fd();
        let file = unsafe { fs::File::from_raw_fd(fd) };

todo!()
        //(self.decoder)(file)
    }
}


#[derive(zbus::DBusError, Debug)]
#[dbus_error(prefix = "org.gnome.glycin.Error")]
pub enum Error {
    #[dbus_error(zbus_error)]
    ZBus(zbus::Error),
    Other(String),
}


