pub fn cors_header(key: &[u8], val: &[u8]) -> tiny_http::Header {
    tiny_http::Header::from_bytes(key, val).unwrap()
}

/// Attach permissive CORS headers to any response. The worker WebView is
/// same-origin with this server so CORS isn't strictly required, but the
/// Canvas login WebView also POSTs here cross-origin — keep it permissive.
/// `Link` is exposed so the scraper can read pagination headers.
pub fn with_cors<R>(resp: tiny_http::Response<R>) -> tiny_http::Response<R>
where
    R: std::io::Read,
{
    resp.with_header(cors_header(b"Access-Control-Allow-Origin", b"*"))
        .with_header(cors_header(
            b"Access-Control-Allow-Methods",
            b"GET, POST, OPTIONS",
        ))
        .with_header(cors_header(b"Access-Control-Allow-Headers", b"Content-Type"))
        .with_header(cors_header(b"Access-Control-Expose-Headers", b"Link"))
}

pub fn cors_response(status: u16) -> tiny_http::Response<std::io::Empty> {
    with_cors(tiny_http::Response::empty(status))
}
