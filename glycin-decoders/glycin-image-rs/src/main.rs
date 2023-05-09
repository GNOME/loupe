use glycin_utils::*;
use std::os::fd::FromRawFd;
use std::os::fd::IntoRawFd;
use std::os::unix::net::UnixStream;

fn main() {
    dbg!("Decoder started");
    async_std::task::block_on(decoder());
}

async fn decoder() {
    let communication = Communication::new().await;
    let file = std::fs::File::open("/etc/os-release").unwrap();
    let fd = dbg!(file.into_raw_fd());
    let frame = Frame {
        width: 1,
        height: 1,
        stride: 3,
        memory_format: MemoryFormat::R8g8b8,
        texture: Texture::MemFd(fd.into()),
        iccp: None.into(),
        cicp: None.into(),
        //pub delay: Optional<Duration>,
    };
    dbg!("send frame");

    communication.send_frame(frame).await;
    std::future::pending::<()>().await;

    dbg!("bye");
}
