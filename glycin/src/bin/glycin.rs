use glycin_utils::*;
use std::ffi::OsString;
use std::io::Read;
use std::io::Seek;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;

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
    let _zbus = zbus::ConnectionBuilder::unix_stream(unix_stream)
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
        dbg!(&message);
        let Texture::MemFd(fd) = message.texture;
        let mfd = memfd::Memfd::try_from_fd(fd.as_raw_fd()).unwrap();

        // 🦭
        mfd.add_seals(&[
            memfd::FileSeal::SealShrink,
            memfd::FileSeal::SealGrow,
            memfd::FileSeal::SealSeal,
        ])
        .unwrap();

        let mut file = mfd.into_file();
        file.rewind();

        let mut mmap = unsafe { memmap::Mmap::map(&file).unwrap() };

        let raw_bytes = unsafe { glib::ffi::g_bytes_new(mmap.as_ptr() as *const _, mmap.len()) };
        let bytes = unsafe { glib::Bytes::from_glib_ptr_borrow(raw_bytes as *const _) };

        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();
        dbg!(buf);
    }
}
