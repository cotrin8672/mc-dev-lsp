use mcdev_companion::transport::{read_frame, write_frame, MAX_FRAME, MAX_HEADER};
use serde_json::json;
use std::io::{self, Cursor};

#[test]
fn byte_exact_utf8_length_and_consecutive_frames() {
    let value = json!({"jsonrpc":"2.0","id":"é😀","result":"日本語"});
    let body = serde_json::to_vec(&value).unwrap();
    let mut bytes = vec![];
    write_frame(&mut bytes, &value).unwrap();
    let prefix = format!("Content-Length: {}\r\n\r\n", body.len());
    assert!(body.len() > String::from_utf8(body.clone()).unwrap().chars().count());
    assert_eq!(bytes, [prefix.as_bytes(), &body].concat());
    let second = json!({"jsonrpc":"2.0","method":"exit"});
    write_frame(&mut bytes, &second).unwrap();
    let mut input = Cursor::new(bytes);
    assert_eq!(read_frame(&mut input).unwrap(), Some(value));
    assert_eq!(read_frame(&mut input).unwrap(), Some(second));
    assert_eq!(read_frame(&mut input).unwrap(), None);
}
#[test]
fn malformed_truncated_duplicate_overflow_and_oversized_frames_are_rejected() {
    for bytes in [
        b"Content-Length: 2\r\n\r\n{".to_vec(),
        b"Content-Length: 2\r\n\r\nxx".to_vec(),
        b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
        b"Content-Length: -1\r\n\r\n".to_vec(),
        b"Content-Length: +2\r\n\r\n{}".to_vec(),
        b"Content-Length: 999999999999999999999\r\n\r\n".to_vec(),
        b"X: value\r\n\r\n{}".to_vec(),
        b"Content-Length: 0\r\n\r\n".to_vec(),
        b"Content-Length: 2\r\n\r\n\xff\xff".to_vec(),
        format!("Content-Length: {}\r\n\r\n", MAX_FRAME + 1).into_bytes(),
        vec![b'a'; MAX_HEADER + 1],
        b"Content-Length: 2".to_vec(),
    ] {
        assert!(read_frame(&mut Cursor::new(bytes)).is_err());
    }
}
#[test]
fn frame_limit_is_enforced_before_writing() {
    let mut bytes = vec![];
    let err = write_frame(&mut bytes, &json!({"result":"x".repeat(MAX_FRAME)})).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    assert!(bytes.is_empty());
}
#[test]
fn header_case_and_lf_are_compatible_with_host() {
    let mut bytes =
        Cursor::new(b"content-length: 2\nContent-Type: application/json\n\n{}".to_vec());
    assert_eq!(read_frame(&mut bytes).unwrap(), Some(json!({})));
}

#[test]
fn single_byte_reads_preserve_split_multibyte_utf8_bodies() {
    let value = json!({"日本語":"é😀"});
    let mut bytes = vec![];
    write_frame(&mut bytes, &value).unwrap();
    let mut input = std::io::BufReader::with_capacity(1, Cursor::new(bytes));
    assert_eq!(read_frame(&mut input).unwrap(), Some(value));
}
