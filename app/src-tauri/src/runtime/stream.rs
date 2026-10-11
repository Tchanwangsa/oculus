//! Streaming a response body into a file, for downloads too large to buffer.

use std::io::{Read, Write};
use std::path::Path;

/// Returned by [`copy_to_file`] when its `cancelled` poll said stop.
pub const CANCELLED: &str = "cancelled";

/// Copy `body` into a new file at `dest` in 256 KiB chunks, reporting the
/// bytes written so far after each and polling `cancelled` before each.
/// `total` is the announced length: a body that ends short of it is refused,
/// since some hosts (and oculus-keyd) end a stream early without an error.
/// The caller removes `dest` on failure.
pub fn copy_to_file(
    body: &mut dyn Read,
    dest: &Path,
    total: Option<u64>,
    on_bytes: &dyn Fn(u64),
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, String> {
    let mut file =
        std::fs::File::create(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    loop {
        if cancelled() {
            return Err(CANCELLED.to_string());
        }
        let n = body
            .read(&mut buf)
            .map_err(|e| format!("network read failed after {done} bytes: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("write failed after {done} bytes: {e}"))?;
        done += n as u64;
        on_bytes(done);
    }
    if let Some(total) = total.filter(|&t| t > 0) {
        if done != total {
            return Err(format!("download incomplete: {done} of {total} bytes"));
        }
    }
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(done)
}

/// Whole percents of `done` over `total`, capped at 100.
pub fn percent(done: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    (done.saturating_mul(100) / total).min(100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn copies_reports_bytes_and_refuses_a_short_body() {
        let dir = crate::test_support::Scratch::new("stream-copy");
        let dest = dir.join("out.bin");
        let seen = Cell::new(0);
        let n = copy_to_file(
            &mut &b"hello"[..],
            &dest,
            Some(5),
            &|d| seen.set(d),
            &|| false,
        )
        .unwrap();
        assert_eq!((n, seen.get()), (5, 5));
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");

        let short = copy_to_file(&mut &b"hel"[..], &dest, Some(5), &|_| {}, &|| false);
        assert_eq!(short.unwrap_err(), "download incomplete: 3 of 5 bytes");
        let unknown = copy_to_file(&mut &b"hel"[..], &dest, None, &|_| {}, &|| false);
        assert_eq!(unknown.unwrap(), 3);
    }

    #[test]
    fn a_cancel_stops_before_the_next_chunk() {
        let dir = crate::test_support::Scratch::new("stream-cancel");
        let dest = dir.join("out.bin");
        let result = copy_to_file(&mut &b"hello"[..], &dest, None, &|_| {}, &|| true);
        assert_eq!(result.unwrap_err(), CANCELLED);
    }

    #[test]
    fn percent_is_capped_and_safe_on_zero() {
        assert_eq!(percent(50, 200), 25);
        assert_eq!(percent(300, 200), 100);
        assert_eq!(percent(1, 0), 0);
    }
}
