//! Internal DBus API

use gdk::prelude::*;
use glycin_utils::*;
use zbus::zvariant;

use std::ffi::OsStr;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;

#[derive(Clone)]
pub struct DecoderProcess<'a> {
    _dbus_connection: zbus::Connection,
    decoding_instruction: DecodingInstructionProxy<'a>,
}

impl<'a> DecoderProcess<'a> {
    pub async fn new() -> DecoderProcess<'a> {
        let (unix_stream, fd_decoder) = std::os::unix::net::UnixStream::pair().unwrap();
        unix_stream
            .set_nonblocking(true)
            .expect("Couldn't set nonblocking");
        fd_decoder
            .set_nonblocking(true)
            .expect("Couldn't set nonblocking");

        let subprocess = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        subprocess.take_fd(fd_decoder, 3);
        let args = [
            "bwrap",
            "--unshare-all",
            "--die-with-parent",
            "--chdir",
            "/",
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "/home/herold/.cargo-target/debug/glycin-image-rs",
        ];
        subprocess.spawn(&args.map(OsStr::new)).unwrap();

        let dbus_connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
            .p2p()
            .server(&zbus::Guid::generate())
            .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
            .build()
            .await
            .unwrap();

        let decoding_instruction = DecodingInstructionProxy::new(&dbus_connection)
            .await
            .expect("Failed to create decoding instruction proxy");

        Self {
            _dbus_connection: dbus_connection,
            decoding_instruction,
        }
    }

    pub async fn init(&self, gfile_worker: GFileWorker) -> Result<ImageInfo, Error> {
        let (remote_reader, writer) = std::os::unix::net::UnixStream::pair().unwrap();

        gfile_worker.write_to(writer);

        self.decoding_instruction
            .init(DecodingRequest {
                fd: unsafe { zvariant::OwnedFd::from_raw_fd(remote_reader.as_raw_fd()) },
            })
            .await
    }

    pub async fn decode_frame(&self) -> Result<gdk::Texture, Error> {
        let frame = self.decoding_instruction.decode_frame().await?;

        // TODO: collect as warning
        crate::icc::apply_transformation(&frame).unwrap();

        let Texture::MemFd(fd) = frame.texture;
        let raw_fd = fd.as_raw_fd();

        let mfd = memfd::Memfd::try_from_fd(fd).unwrap();
        // 🦭
        mfd.add_seals(&[
            memfd::FileSeal::SealShrink,
            memfd::FileSeal::SealGrow,
            memfd::FileSeal::SealWrite,
            memfd::FileSeal::SealSeal,
        ])
        .unwrap();

        let bytes: glib::Bytes = unsafe {
            let mmap = glib::ffi::g_mapped_file_new_from_fd(
                raw_fd,
                glib::ffi::GFALSE,
                std::ptr::null_mut(),
            );
            glib::translate::from_glib_full(glib::ffi::g_mapped_file_get_bytes(mmap))
        };

        let texture = gdk::MemoryTexture::new(
            frame.width.try_into().unwrap(),
            frame.height.try_into().unwrap(),
            gdk_memory_format(frame.memory_format),
            &bytes,
            frame.stride.try_into().unwrap(),
        );

        Ok(texture.upcast())
    }
}

use std::io::Write;
const BUF_SIZE: usize = u16::MAX as usize;

#[zbus::dbus_proxy(
    interface = "org.gnome.glycin.DecodingInstruction",
    default_path = "/org/gnome/glycin"
)]
trait DecodingInstruction {
    async fn init(&self, message: DecodingRequest) -> Result<ImageInfo, Error>;
    async fn decode_frame(&self) -> Result<Frame, Error>;
}

const fn gdk_memory_format(format: MemoryFormat) -> gdk::MemoryFormat {
    match format {
        MemoryFormat::L8 => unimplemented!(),
        MemoryFormat::L8a8 => unimplemented!(),
        MemoryFormat::L16 => unimplemented!(),
        MemoryFormat::L16a16 => unimplemented!(),
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
    }
}

pub struct GFileWorker {
    writer_send: Sender<UnixStream>,
    first_bytes_recv: Receiver<Vec<u8>>,
}
use async_std::channel::{Receiver, Sender};
impl GFileWorker {
    pub fn spawn(file: gio::File) -> GFileWorker {
        let cancellable = gio::Cancellable::new();

        let (writer_send, writer_recv) = async_std::channel::bounded(1);
        let (first_bytes_send, first_bytes_recv) = async_std::channel::bounded(1);

        std::thread::spawn(move || {
            let mut reader = file.read(Some(&cancellable)).unwrap().into_read();
            let mut buf = vec![0; BUF_SIZE];

            let n = reader.read(&mut buf).unwrap();
            first_bytes_send.try_send(buf[..n].to_vec()).unwrap();

            let mut writer: UnixStream = async_std::task::block_on(writer_recv.recv()).unwrap();

            writer.write_all(&buf[..n]).unwrap();

            loop {
                let n = reader.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                writer.write_all(&buf[..n]).unwrap();
            }
        });

        GFileWorker {
            writer_send,
            first_bytes_recv,
        }
    }

    pub fn write_to(self, stream: UnixStream) {
        self.writer_send.try_send(stream).unwrap();
    }

    pub async fn head(&self) -> Vec<u8> {
        self.first_bytes_recv.recv().await.unwrap()
    }
}