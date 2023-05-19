//! Internal DBus API

use futures::channel::oneshot;
use futures::future;
use futures::FutureExt;
use gdk::prelude::*;
use glycin_utils::*;
use zbus::zvariant;

use std::ffi::OsStr;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct DecoderProcess<'a> {
    _dbus_connection: zbus::Connection,
    decoding_instruction: DecodingInstructionProxy<'a>,
    mime_type: String,
}

impl<'a> DecoderProcess<'a> {
    pub async fn new(
        mime_type: &glib::GString,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<DecoderProcess<'a>, Error> {
        let decoders = std::collections::HashMap::from([
            (
                "image/jpeg",
                "/home/herold/.cargo-target/release/glycin-image-rs",
            ),
            (
                "image/png",
                "/home/herold/.cargo-target/release/glycin-image-rs",
            ),
        ]);

        let decoder = decoders.get(mime_type.as_str()).expect("TODO");

        let (unix_stream, fd_decoder) = std::os::unix::net::UnixStream::pair()?;
        unix_stream
            .set_nonblocking(true)
            .expect("Couldn't set nonblocking");
        fd_decoder
            .set_nonblocking(true)
            .expect("Couldn't set nonblocking");

        let subprocess_launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        subprocess_launcher.take_fd(fd_decoder, 3);
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
            decoder,
        ];
        let subprocess = subprocess_launcher.spawn(&args.map(OsStr::new))?;

        if let Some(cancellable) = cancellable {
            cancellable.connect_cancelled_local(move |_| subprocess.force_exit());
        }

        let dbus_connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
            .p2p()
            .server(&zbus::Guid::generate())
            .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
            .build()
            .await?;

        let decoding_instruction = DecodingInstructionProxy::new(&dbus_connection)
            .await
            .expect("Failed to create decoding instruction proxy");

        Ok(Self {
            _dbus_connection: dbus_connection,
            decoding_instruction,
            mime_type: mime_type.to_string(),
        })
    }

    pub async fn init(&self, gfile_worker: GFileWorker) -> Result<ImageInfo, Error> {
        let (remote_reader, writer) = std::os::unix::net::UnixStream::pair()?;

        gfile_worker.write_to(writer)?;

        let fd = unsafe { zvariant::OwnedFd::from_raw_fd(remote_reader.as_raw_fd()) };
        let mime_type = self.mime_type.clone();

        let image_info = self
            .decoding_instruction
            .init(DecodingRequest { fd, mime_type })
            .shared();

        let reader_error = gfile_worker.error();
        futures::pin_mut!(reader_error);

        futures::select! {
            res = image_info.clone() => res.map(|_| ()).map_err(Into::into),
            res = reader_error.fuse() => res,
        }?;

        image_info.await.map_err(Into::into)
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
    async fn init(&self, message: DecodingRequest) -> Result<ImageInfo, RemoteError>;
    async fn decode_frame(&self) -> Result<Frame, RemoteError>;
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
    file: gio::File,
    writer_send: Mutex<Option<oneshot::Sender<UnixStream>>>,
    first_bytes_recv: future::Shared<oneshot::Receiver<Arc<Vec<u8>>>>,
    error_recv: future::Shared<oneshot::Receiver<Result<(), Error>>>,
}
use std::sync::Mutex;
impl GFileWorker {
    pub fn spawn(file: gio::File, cancellable: Option<gio::Cancellable>) -> GFileWorker {
        let gfile = file.clone();

        let (error_send, error_recv) = oneshot::channel();
        let (first_bytes_send, first_bytes_recv) = oneshot::channel();
        let (writer_send, writer_recv) = oneshot::channel();

        std::thread::spawn(move || {
            Self::handle_errors(error_send, move || {
                let reader = gfile.read(cancellable.as_ref())?;
                let mut buf = vec![0; BUF_SIZE];

                let n = reader.read(&mut buf, cancellable.as_ref())?;
                let first_bytes = Arc::new(buf[..n].to_vec());
                first_bytes_send
                    .send(first_bytes.clone())
                    .or(Err(Error::InternalCommunicationCanceled))?;

                let mut writer: UnixStream = async_std::task::block_on(writer_recv)?;

                writer.write_all(&first_bytes)?;
                drop(first_bytes);

                loop {
                    let n = reader.read(&mut buf, cancellable.as_ref())?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n])?;
                }

                Ok(())
            })
        });

        GFileWorker {
            file,
            writer_send: Mutex::new(Some(writer_send)),
            first_bytes_recv: first_bytes_recv.shared(),
            error_recv: error_recv.shared(),
        }
    }

    fn handle_errors(
        error_send: oneshot::Sender<Result<(), Error>>,
        f: impl FnOnce() -> Result<(), Error>,
    ) {
        let result = f();
        let _result = error_send.send(result);
    }

    pub fn write_to(&self, stream: UnixStream) -> Result<(), Error> {
        let sender = std::mem::take(&mut *self.writer_send.lock().unwrap());

        sender
            // TODO: this fails if write_to is called a second time
            .unwrap()
            .send(stream)
            .or(Err(Error::InternalCommunicationCanceled))
    }

    pub fn file(&self) -> &gio::File {
        &self.file
    }

    pub async fn error(&self) -> Result<(), Error> {
        match self.error_recv.clone().await {
            Ok(result) => result,
            Err(_) => Ok(()),
        }
    }

    pub async fn head(&self) -> Result<Arc<Vec<u8>>, Error> {
        futures::select!(
            err = self.error_recv.clone() => err?,
            _bytes = self.first_bytes_recv.clone() => Ok(()),
        )?;

        match self.first_bytes_recv.clone().await {
            Err(_) => self.error_recv.clone().await?.map(|_| Default::default()),
            Ok(bytes) => Ok(bytes),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    RemoteError(RemoteError),
    GLibError(glib::Error),
    StdIoError(Arc<std::io::Error>),
    InternalCommunicationCanceled,
}

impl From<RemoteError> for Error {
    fn from(err: RemoteError) -> Self {
        Self::RemoteError(err)
    }
}

impl From<glib::Error> for Error {
    fn from(err: glib::Error) -> Self {
        Self::GLibError(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::StdIoError(Arc::new(err))
    }
}

impl From<oneshot::Canceled> for Error {
    fn from(_err: oneshot::Canceled) -> Self {
        Self::InternalCommunicationCanceled
    }
}

impl From<zbus::Error> for Error {
    fn from(err: zbus::Error) -> Self {
        Self::RemoteError(RemoteError::ZBus(err))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> StdResult<(), std::fmt::Error> {
        match self {
            Self::RemoteError(err) => write!(f, "{err}"),
            Self::GLibError(err) => write!(f, "{err}"),
            Self::StdIoError(err) => write!(f, "{err}"),
            Self::InternalCommunicationCanceled => {
                write!(f, "Internal communication was unexpectedly canceled")
            }
        }
    }
}

impl std::error::Error for Error {}

pub type StdResult<T, E> = std::result::Result<T, E>;
