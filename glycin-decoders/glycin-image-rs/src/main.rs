use glycin_utils::*;
use std::os::fd::IntoRawFd;
use std::io::Write;

fn main() {
    dbg!("Decoder started");
    async_std::task::block_on(decoder());
}

async fn decoder() {
    let communication = Communication::new().await;
    let path = "/home/herold/loupetest/DSCN0029.jpg";

    let file = std::fs::File::open(path).unwrap();

    let decoder = image::codecs::jpeg::JpegDecoder::new(file).unwrap();

    let (frame, _memory) = Frame::from_decoder(decoder);

    communication.send_frame(frame).await;
    std::future::pending::<()>().await;
}
