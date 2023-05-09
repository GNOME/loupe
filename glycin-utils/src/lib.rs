use serde::{Deserialize, Serialize};
//use std::num::NonZeroU32;
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
//use std::time::Duration;
use zbus::zvariant::{self, Optional, Type};

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
    MemFd(zvariant::Fd),
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
        self
            .decoding_update
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
