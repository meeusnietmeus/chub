use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

fn socket_path() -> String {
    let uid = rustix::process::getuid().as_raw();
    format!("/run/user/{}/{}.sock", uid, env!("CARGO_PKG_NAME"))
}

/// Client side: send toggle to running daemon
pub fn send_toggle() {
    let path = socket_path();
    match UnixStream::connect(&path) {
        Ok(mut stream) => {
            if let Err(e) = stream.write_all(&[b't']) {
                eprintln!("failed to send toggle: {}", e);
            }
        }
        Err(_) => {
            eprintln!("{} daemon is not running", env!("CARGO_PKG_NAME"));
            std::process::exit(1);
        }
    }
}

/// Server side: bind socket, returns listener ready for calloop
pub fn bind_socket() -> UnixListener {
    let path = socket_path();
    // Remove stale socket from previous run
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("Failed to bind IPC socket");
    listener.set_nonblocking(true).expect("Failed to set nonblocking");
    listener
}
