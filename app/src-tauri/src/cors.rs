pub fn cors_header(key: &[u8], val: &[u8]) -> tiny_http::Header {
    tiny_http::Header::from_bytes(key, val).unwrap()
}

pub fn cors_response(status: u16) -> tiny_http::Response<std::io::Empty> {
    tiny_http::Response::empty(status)
        .with_header(cors_header(b"Access-Control-Allow-Origin", b"*"))
        .with_header(cors_header(b"Access-Control-Allow-Methods", b"POST, OPTIONS"))
        .with_header(cors_header(b"Access-Control-Allow-Headers", b"Content-Type"))
}
