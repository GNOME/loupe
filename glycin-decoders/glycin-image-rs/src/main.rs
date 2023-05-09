use glycin_utils::*;
use std::os::fd::IntoRawFd;
use std::io::Write;

fn main() {
    dbg!("Decoder started");
    async_std::task::block_on(decoder());
}

async fn decoder() {
    let communication = Communication::new().await;

    let memfd = memfd::MemfdOptions::default().allow_sealing(true).create("xyz").unwrap();
    let mut file = memfd.into_file();
    file.set_len(16).unwrap();

    let mut mmap = unsafe { memmap::MmapMut::map_mut(&file).unwrap() };
    mmap[0] = 66;

    drop(mmap);

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
    communication.send_frame(frame).await;
    std::future::pending::<()>().await;
}
