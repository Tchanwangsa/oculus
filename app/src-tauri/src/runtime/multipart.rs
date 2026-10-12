//! `multipart/form-data` bodies by hand: ureq 2 has none. Text fields, then
//! one file part, which is all Groq, a local MinerU and Ed's workspace server
//! take. The envelope is split so a caller can stream the file between its
//! halves.

use std::sync::atomic::{AtomicU64, Ordering};

/// The one file part's headers.
pub(crate) struct FilePart<'a> {
    pub field: &'a str,
    pub filename: &'a str,
    pub content_type: &'a str,
}

/// Unique among concurrent requests without a random source.
pub(crate) fn boundary() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = crate::runtime::clock::now_nanos() as u64;
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("oculus{nanos:016x}{:08x}{sequence:08x}", std::process::id())
}

/// The request's `Content-Type` for `boundary`.
pub(crate) fn content_type(boundary: &str) -> String {
    format!("multipart/form-data; boundary={boundary}")
}

/// Everything before the file's bytes, and everything after them.
pub(crate) fn envelope(
    boundary: &str,
    fields: &[(&str, &str)],
    file: &FilePart,
) -> (Vec<u8>, Vec<u8>) {
    let mut head = String::new();
    for (name, value) in fields {
        head.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    head.push_str(&format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n\
         Content-Type: {}\r\n\r\n",
        file.field,
        header_safe(file.filename),
        file.content_type
    ));
    (
        head.into_bytes(),
        format!("\r\n--{boundary}--\r\n").into_bytes(),
    )
}

/// The whole body, for a file already in memory.
pub(crate) fn body(
    boundary: &str,
    fields: &[(&str, &str)],
    file: &FilePart,
    bytes: &[u8],
) -> Vec<u8> {
    let (head, tail) = envelope(boundary, fields, file);
    let mut body = Vec::with_capacity(head.len() + bytes.len() + tail.len());
    body.extend_from_slice(&head);
    body.extend_from_slice(bytes);
    body.extend_from_slice(&tail);
    body
}

/// Drop the characters that could end a quoted header value early. Dropped,
/// not escaped: RFC 2183 escaping is read inconsistently.
fn header_safe(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '"' | '\\' | '\r' | '\n'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: FilePart = FilePart {
        field: "file",
        filename: "audio.ogg",
        content_type: "audio/ogg",
    };

    #[test]
    fn the_body_has_each_field_then_the_file() {
        let body = body(
            "B",
            &[("model", "whisper-large-v3-turbo"), ("temperature", "0")],
            &FILE,
            b"\x00OggS\xff",
        );
        let mut expected = b"--B\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n\
whisper-large-v3-turbo\r\n\
--B\r\nContent-Disposition: form-data; name=\"temperature\"\r\n\r\n0\r\n\
--B\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.ogg\"\r\n\
Content-Type: audio/ogg\r\n\r\n"
            .to_vec();
        expected.extend_from_slice(b"\x00OggS\xff");
        expected.extend_from_slice(b"\r\n--B--\r\n");
        assert_eq!(body, expected);
    }

    #[test]
    fn the_envelope_brackets_the_file_and_keeps_the_name_quoted() {
        let file = FilePart {
            filename: "Lecture \"3\".pdf",
            ..FILE
        };
        let (head, tail) = envelope("BOUND", &[], &file);
        let head = String::from_utf8(head).unwrap();
        assert!(head.starts_with("--BOUND\r\n"), "{head}");
        assert!(head.contains("filename=\"Lecture 3.pdf\""), "{head}");
        assert!(head.ends_with("\r\n\r\n"), "{head}");
        assert_eq!(String::from_utf8(tail).unwrap(), "\r\n--BOUND--\r\n");
    }

    #[test]
    fn boundaries_differ_between_calls() {
        assert_ne!(boundary(), boundary());
        assert_eq!(content_type("B"), "multipart/form-data; boundary=B");
    }
}
