//! How a transport failure reads in an error message.

/// A transport failure as one line: kind, message, then the source chain,
/// which is where the cause lives ("certificate expired", "connection reset").
/// Never the URL — a signed one carries its signature in the query.
pub fn transport_detail(transport: &ureq::Transport) -> String {
    let mut detail = transport.kind().to_string();
    if let Some(message) = transport.message() {
        detail.push_str(": ");
        detail.push_str(message);
    }
    let mut source = std::error::Error::source(transport);
    while let Some(cause) = source {
        detail.push_str(": ");
        detail.push_str(&cause.to_string());
        source = cause.source();
    }
    detail
}
