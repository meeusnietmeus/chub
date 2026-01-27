use std::fs::OpenOptions;
use std::io::Write;
use rustix::fs::{flock, FlockOperation};

pub fn acquire_lock() -> Option<std::fs::File> {
    let uid = rustix::process::getuid().as_raw();
    let lock_path = format!("/run/user/{}/{}.lock", uid, env!("CARGO_PKG_NAME"));

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .expect("Failed to open instance lock file");

    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {
            let pid = rustix::process::getpid().as_raw_nonzero().get();
            write!(file, "{}", pid).expect("Failed to write PID");
            file.flush().expect("Failed to flush lock file");
            Some(file)
        }
        Err(_) => None,
    }
}
