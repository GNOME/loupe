use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
fn main() {
    println!("hey");
    async_std::task::block_on(xmain());
}

async fn xmain() {
    let unix_stream = unsafe { UnixStream::from_raw_fd(3) };

    println!("Hello, world!");
    let connection = zbus::ConnectionBuilder::unix_stream(unix_stream)
        .p2p()
        .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
        .build()
        .await
        .unwrap();

            let proxy = MyGreeterProxy::new(&connection).await.unwrap();
    let reply = proxy.say_hello("Maria").await.unwrap();
    println!("{reply}");
}

use zbus::{Connection, Result, dbus_proxy};

#[zbus::dbus_proxy(
    interface = "org.gnome.glycin",
    default_path = "/org/gnome/glycin"
)]
trait MyGreeter {
    async fn say_hello(&self, name: &str) -> Result<String>;
}

