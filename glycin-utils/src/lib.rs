pub mod image_rs;

use serde::{Deserialize, Serialize};
//use std::num::NonZeroU32;
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
//use std::time::Duration;
use std::ffi::CString;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::os::fd::AsFd;
use std::os::fd::IntoRawFd;
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
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

impl<'a> Communication<'a> {
    pub async fn new() -> Communication<'a> {
        let unix_stream = unsafe { UnixStream::from_raw_fd(3) };

        let dbus_connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
            .p2p()
            .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
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
