use glycin_utils::*;
use std::ffi::OsString;
use std::io::Read;
use std::io::Seek;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;

use gtk4::prelude::*;

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
    async fn send_frame(&self, frame: Frame) {
        dbg!(&frame);
        let Texture::MemFd(fd) = frame.texture;
        let mfd = memfd::Memfd::try_from_fd(fd.as_raw_fd()).unwrap();

        // 🦭
        mfd.add_seals(&[
            memfd::FileSeal::SealShrink,
            memfd::FileSeal::SealGrow,
            memfd::FileSeal::SealSeal,
        ])
        .unwrap();

        let mut file = mfd.into_file();
        file.rewind().unwrap();

        let mmap = unsafe { memmap::Mmap::map(&file).unwrap() };

        dbg!(mmap.len());
        dbg!(mmap[0]);

        /*
        let raw_bytes = unsafe { glib::ffi::g_bytes_new_static(mmap.as_ptr() as *const _, mmap.len()) };
        let bytes = unsafe { glib::Bytes::from_glib_ptr_borrow(raw_bytes as *const _) };
        */

             let bytes :glib::Bytes =   unsafe {
            glib::translate::from_glib_full(glib::ffi::g_bytes_new_static(
                mmap.as_ptr() as *const _,
                mmap.len(),
            ))
        };

        dbg!(bytes[0]);

        let texture = gdk::MemoryTexture::new(
            frame.width.try_into().unwrap(),
            frame.height.try_into().unwrap(),
            gdk_memory_format(frame.memory_format),
            &bytes,
            frame.stride.try_into().unwrap(),
        );

        gtk4::init();
        let snapshot = gtk4::Snapshot::new();
        dbg!("creating snapshot");
        texture.snapshot(&snapshot, texture.width() as f64, texture.height() as f64);
        dbg!("snapshot created");
        snapshot.to_node().unwrap().write_to_file("/home/herold/node.node").unwrap();
        dbg!("snapshot written");

        //let mut buf = String::new();
        //file.read_to_string(&mut buf).unwrap();
        //dbg!(buf);
    }
}

fn gdk_memory_format(format: MemoryFormat) -> gdk::MemoryFormat {
    match format {
        MemoryFormat::B8g8r8a8Premultiplied => gdk::MemoryFormat::B8g8r8a8Premultiplied,
        MemoryFormat::A8r8g8b8Premultiplied => gdk::MemoryFormat::A8r8g8b8Premultiplied,
        MemoryFormat::R8g8b8a8Premultiplied => gdk::MemoryFormat::R8g8b8a8Premultiplied,
        MemoryFormat::B8g8r8a8 => gdk::MemoryFormat::B8g8r8a8,
        MemoryFormat::A8r8g8b8 => gdk::MemoryFormat::A8r8g8b8,
        MemoryFormat::R8g8b8a8 => gdk::MemoryFormat::R8g8b8a8,
        MemoryFormat::A8b8g8r8 => gdk::MemoryFormat::A8b8g8r8,
        MemoryFormat::R8g8b8 => gdk::MemoryFormat::R8g8b8,
        MemoryFormat::B8g8r8 => gdk::MemoryFormat::B8g8r8,
        MemoryFormat::R16g16b16 => gdk::MemoryFormat::R16g16b16,
        MemoryFormat::R16g16b16a16Premultiplied => gdk::MemoryFormat::R16g16b16a16Premultiplied,
        MemoryFormat::R16g16b16a16 => gdk::MemoryFormat::R16g16b16a16,
        MemoryFormat::R16g16b16Float => gdk::MemoryFormat::R16g16b16Float,
        MemoryFormat::R16g16b16a16Float => gdk::MemoryFormat::R16g16b16a16Float,
        MemoryFormat::R32g32b32Float => gdk::MemoryFormat::R32g32b32Float,
        MemoryFormat::R32g32b32a32FloatPremultiplied => {
            gdk::MemoryFormat::R32g32b32a32FloatPremultiplied
        }
        MemoryFormat::R32g32b32a32Float => gdk::MemoryFormat::R32g32b32a32Float,
        MemoryFormat::L8 => unimplemented!(),
        MemoryFormat::L8a8 => unimplemented!(),
        MemoryFormat::L16 => unimplemented!(),
        MemoryFormat::L16a16 => unimplemented!(),
    }
}
