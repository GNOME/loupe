use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
use std::time::Duration;
use zbus::zvariant::{Optional, Type};

#[derive(Deserialize, Serialize, Type)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub exif: Optional<Vec<u8>>,
    pub xmp: Optional<Vec<u8>>,
    pub transformations_applied: bool,
}

#[derive(Deserialize, Serialize, Type)]
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

#[derive(Deserialize, Serialize, Type)]
pub enum Texture {
    MemFd(u32),
}

#[derive(Deserialize, Serialize, Type)]
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
    R16g16b16a16FloatPremultiplied,
    R16g16b16a16Float,
    R32g32b32Float,
    R32g32b32a32FloatPremultiplied,
    R32g32b32a32Float,
}

pub struct Communication<'a> {
    dbus_connection: zbus::Connection,
    decoding_update: DecodingUpdateProxy<'a>,
}

impl<'a> Communication<'a> {
    async fn new() -> zbus::Result<Communication<'a>> {
        let unix_stream = unsafe { UnixStream::from_raw_fd(3) };

        let dbus_connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
            .p2p()
            .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
            .build()
            .await?;

        let decoding_update = DecodingUpdateProxy::new(&dbus_connection).await?;

        Ok(Communication {
            dbus_connection,
            decoding_update,
        })
    }

    async fn send_image_info(&self, message: ImageInfo) {
        self.decoding_update
            .send_image_info(message)
            .await
            .expect("Failed to send image info");
    }
}

#[zbus::dbus_proxy(interface = "org.gnome.glycin.in", default_path = "/org/gnome/glycin")]
trait DecodingUpdate {
    async fn send_image_info(&self, message: ImageInfo) -> zbus::Result<()>;
    async fn send_frame(&self, message: Frame) -> zbus::Result<()>;
}
