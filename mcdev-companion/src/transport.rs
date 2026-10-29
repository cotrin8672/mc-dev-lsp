use serde_json::Value;
use std::io::{self, BufRead, Read, Write};

pub const MAX_FRAME: usize = 1024 * 1024;
pub const MAX_HEADER: usize = 4096;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    Ok(read_frame_with_length(reader)?.map(|(value, _)| value))
}

/// Returns the actual JSON body length, excluding Content-Length framing.
pub fn read_frame_with_length(reader: &mut impl BufRead) -> io::Result<Option<(Value, usize)>> {
    let mut length = None;
    let mut header_bytes = 0;
    loop {
        let mut line = String::new();
        let count = reader.take((MAX_HEADER + 1) as u64).read_line(&mut line)?;
        if count == 0 && header_bytes == 0 {
            return Ok(None);
        }
        header_bytes += count;
        if count == 0 || header_bytes > MAX_HEADER || !line.ends_with('\n') {
            return Err(invalid("invalid frame header"));
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let (name, value) = line
            .trim_end()
            .split_once(':')
            .ok_or_else(|| invalid("invalid frame header"))?;
        if name.eq_ignore_ascii_case("Content-Length") {
            if length.is_some() {
                return Err(invalid("duplicate Content-Length"));
            }
            let value = value.trim();
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid("invalid Content-Length"));
            }
            length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| invalid("invalid Content-Length"))?,
            );
        }
    }
    let length = length
        .filter(|n| *n > 0 && *n <= MAX_FRAME)
        .ok_or_else(|| invalid("missing or oversized Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(|value| Some((value, length)))
        .map_err(|_| invalid("invalid JSON frame"))
}

pub fn write_frame(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(message)?;
    if body.len() > MAX_FRAME {
        return Err(invalid("outgoing frame too large"));
    }
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}
