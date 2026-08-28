//! Framed stdin transport for one Hook JSON payload.

use std::io::{self, Read};

const HOOK_STDIN_CHUNK_BYTES: usize = 16 * 1024;
const HOOK_STDIN_MAX_BYTES: usize = 1024 * 1024;

pub(in super::super) fn read_hook_stdin_bounded() -> io::Result<String> {
    let mut stdin = io::stdin().lock();
    read_one_json_frame(&mut stdin)
}

fn read_one_json_frame(mut reader: impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0; HOOK_STDIN_CHUNK_BYTES];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        append_chunk(&mut bytes, &chunk[..read])?;
        if json_frame_is_complete(&bytes)? {
            break;
        }
    }

    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

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

fn json_frame_is_complete(bytes: &[u8]) -> io::Result<bool> {
    let mut stream = serde_json::Deserializer::from_slice(bytes).into_iter::<serde_json::Value>();
    match stream.next() {
        Some(Ok(_)) => {
            let trailing = &bytes[stream.byte_offset()..];
            if trailing.iter().all(u8::is_ascii_whitespace) {
                Ok(true)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "hook stdin contains more than one JSON frame",
                ))
            }
        }
        Some(Err(error)) if error.is_eof() => Ok(false),
        Some(Err(error)) => Err(io::Error::new(io::ErrorKind::InvalidData, error)),
        None => Ok(false),
    }
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_runtime_stdin.rs"]
mod tests;
