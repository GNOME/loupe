use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
fn main() {
    println!("hey");
    async_std::task::block_on(xmain());
}

async fn xmain() {
    let unix_stream = unsafe { UnixStream::from_raw_fd(3) };

    println!("Hello, world!");
    let zbus = zbus::ConnectionBuilder::unix_stream(unix_stream)
        .p2p()
        .auth_mechanisms(&[zbus::AuthMechanism::Anonymous])
        .build()
        .await
        .unwrap();
}
