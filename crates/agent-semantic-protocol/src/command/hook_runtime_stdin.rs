//! Bounded stdin transport for hook payloads.

use std::cell::RefCell;
use std::io;
#[cfg(not(unix))]
use std::io::Read;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
const HOOK_STDIN_FIRST_BYTE_TIMEOUT: Duration = Duration::from_millis(50);
#[cfg(unix)]
const HOOK_STDIN_IDLE_TIMEOUT: Duration = Duration::from_millis(10);
#[cfg(unix)]
const HOOK_STDIN_POLL_INTERVAL: Duration = Duration::from_millis(1);
#[cfg(unix)]
const HOOK_STDIN_CHUNK_BYTES: usize = 16 * 1024;
#[cfg(unix)]
const HOOK_STDIN_MAX_BYTES: usize = 1024 * 1024;

thread_local! {
    static HOOK_STDIN_OVERRIDE: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) fn with_hook_stdin_override<T>(input: String, operation: impl FnOnce() -> T) -> T {
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            HOOK_STDIN_OVERRIDE.with(|slot| {
                slot.replace(self.0.take());
            });
        }
    }

    let previous = HOOK_STDIN_OVERRIDE.with(|slot| slot.replace(Some(input)));
    let _restore = Restore(previous);
    operation()
}

fn take_hook_stdin_override() -> Option<String> {
    HOOK_STDIN_OVERRIDE.with(|slot| slot.borrow_mut().take())
}

#[cfg(unix)]
pub(super) fn read_hook_stdin_bounded() -> io::Result<String> {
    if let Some(input) = take_hook_stdin_override() {
        if input.len() > HOOK_STDIN_MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("hook payload exceeds {HOOK_STDIN_MAX_BYTES} bytes"),
            ));
        }
        return Ok(input);
    }
    use std::os::fd::AsRawFd;

    let stdin = io::stdin();
    let fd = stdin.as_raw_fd();
    let original_flags = fcntl_get_flags(fd)?;
    fcntl_set_flags(fd, original_flags | libc::O_NONBLOCK)?;
    let _restore = StdinFlagsRestore { fd, original_flags };

    let mut bytes = Vec::new();
    let start = Instant::now();
    let mut last_read = None;
    let mut chunk = vec![0; HOOK_STDIN_CHUNK_BYTES];

    loop {
        match read_stdin_fd(fd, &mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                append_chunk(&mut bytes, &chunk[..read])?;
                last_read = Some(Instant::now());
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if stdin_read_deadline_elapsed(start, last_read) {
                    break;
                }
                std::thread::sleep(HOOK_STDIN_POLL_INTERVAL);
            }
            Err(error) => return Err(error),
        }
    }

    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(not(unix))]
pub(super) fn read_hook_stdin_bounded() -> io::Result<String> {
    if let Some(input) = take_hook_stdin_override() {
        return Ok(input);
    }
    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    Ok(stdin)
}

#[cfg(unix)]
struct StdinFlagsRestore {
    fd: i32,
    original_flags: i32,
}

#[cfg(unix)]
impl Drop for StdinFlagsRestore {
    fn drop(&mut self) {
        let _ = fcntl_set_flags(self.fd, self.original_flags);
    }
}

#[cfg(unix)]
fn fcntl_get_flags(fd: i32) -> io::Result<i32> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(flags)
    }
}

#[cfg(unix)]
fn fcntl_set_flags(fd: i32, flags: i32) -> io::Result<()> {
    let status = unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
    if status == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn read_stdin_fd(fd: i32, chunk: &mut [u8]) -> io::Result<usize> {
    let read = unsafe { libc::read(fd, chunk.as_mut_ptr().cast(), chunk.len()) };
    if read >= 0 {
        return Ok(read as usize);
    }
    Err(io::Error::last_os_error())
}

#[cfg(unix)]
fn append_chunk(bytes: &mut Vec<u8>, chunk: &[u8]) -> io::Result<()> {
    if bytes.len() + chunk.len() > HOOK_STDIN_MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("hook payload exceeds {HOOK_STDIN_MAX_BYTES} bytes"),
        ));
    }
    bytes.extend(chunk);
    Ok(())
}

#[cfg(unix)]
fn stdin_read_deadline_elapsed(start: Instant, last_read: Option<Instant>) -> bool {
    let now = Instant::now();
    match last_read {
        Some(last_read) => now.duration_since(last_read) >= HOOK_STDIN_IDLE_TIMEOUT,
        None => now.duration_since(start) >= HOOK_STDIN_FIRST_BYTE_TIMEOUT,
    }
}
