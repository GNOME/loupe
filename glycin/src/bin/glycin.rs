use gio::prelude::*;
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

    let greeter = DecodingUpdate { count: 0 };
    let zbus = zbus::ConnectionBuilder::unix_stream(unix_stream)
        .p2p()
        .server(&zbus::Guid::generate())
        .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
        .serve_at("/org/gnome/glycin", greeter)
        .unwrap()
        .build()
        .await
        .unwrap();

    std::future::pending::<()>().await;
}

struct DecodingUpdate {
    count: u64,
}

#[zbus::dbus_interface(name = "org.gnome.glycin")]
impl DecodingUpdate {
    // Can be `async` as well.
    fn say_hello(&mut self, name: &str) -> String {
        self.count += 1;
        format!("Hello {}! I have been called {} times.", name, self.count)
    }
}
