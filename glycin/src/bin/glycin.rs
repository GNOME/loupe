use gio::prelude::*;
use glycin_utils::*;
use std::ffi::OsString;

fn main() {
    println!("1");
    async_std::task::block_on(xmain());
}

async fn xmain() {
    println!("2");
    let (unix_stream, fd_decoder) = std::os::unix::net::UnixStream::pair().unwrap();
    unix_stream
        .set_nonblocking(true)
        .expect("Couldn't set nonblocking");
    fd_decoder
        .set_nonblocking(true)
        .expect("Couldn't set nonblocking");

    let subprocess = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
    subprocess.take_fd(fd_decoder, 3);
    subprocess
        .spawn(&[&OsString::from(
            "/home/herold/.cargo-target/debug/glycin-image-rs",
        )])
        .unwrap();

    let update = DecodingUpdate;
    let zbus = zbus::ConnectionBuilder::unix_stream(unix_stream)
        .p2p()
        .server(&zbus::Guid::generate())
        .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
        .serve_at("/org/gnome/glycin", update)
        .unwrap()
        .build()
        .await
        .unwrap();

    dbg!("waiting");
    std::future::pending::<()>().await;
    dbg!("bye");
}

struct DecodingUpdate;

#[zbus::dbus_interface(name = "org.gnome.glycin.DecodingUpdate")]
impl DecodingUpdate {
    async fn send_image_info(&self, message: ImageInfo) {
        dbg!(message);
    }
    async fn send_frame(&self, message: Frame) {
        dbg!(message);
    }
}
