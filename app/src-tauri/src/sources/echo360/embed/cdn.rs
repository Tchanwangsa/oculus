//! Requests to Echo360's CDN with signed URLs. No cookie and no Bearer: the
//! signature in the query is the whole credential. Errors name what was asked
//! for, never the URL, which carries the signature.

use std::io::Read;

fn call(request: ureq::Request, what: &str) -> Result<ureq::Response, String> {
    match request.call() {
        Ok(resp) => Ok(resp),
        Err(ureq::Error::Status(code, _)) => {
            Err(format!("Echo360's CDN answered HTTP {code} for {what}"))
        }
        Err(e) => Err(format!(
            "Echo360's CDN could not be reached for {what}: {e}"
        )),
    }
}

fn length(resp: &ureq::Response) -> Option<u64> {
    resp.header("content-length")
        .and_then(|s| s.trim().parse().ok())
}

pub fn get_text(url: &str, what: &str) -> Result<String, String> {
    call(ureq::get(url), what)?
        .into_string()
        .map_err(|e| format!("{what} could not be read: {e}"))
}

pub fn get_bytes(url: &str, what: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    call(ureq::get(url), what)?
        .into_reader()
        .read_to_end(&mut out)
        .map_err(|e| format!("{what} could not be read: {e}"))?;
    Ok(out)
}

/// The announced size from a HEAD, or `None` when it gives none or fails;
/// only progress depends on it, and the GET that follows reports its own.
pub fn head_length(url: &str) -> Option<u64> {
    ureq::head(url).call().ok().as_ref().and_then(length)
}

/// A GET whose body is still on the wire, with its announced length.
pub fn open(url: &str, what: &str) -> Result<(Box<dyn Read + Send>, Option<u64>), String> {
    let resp = call(ureq::get(url), what)?;
    let total = length(&resp);
    Ok((resp.into_reader(), total))
}
