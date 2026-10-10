use std::io::{Read, Write};
use std::path::Path;

use super::transport::{self, is_canvas, resolve, Streamed};
use super::{Canvas, CanvasError, CANCELLED};

impl Canvas {
    /// Stream a GET into `dest` without holding the body in memory, for files
    /// too large to buffer. A Canvas URL goes through oculus-keyd, which
    /// attaches the cookie; the signed file host a Canvas download redirects
    /// to is fetched directly and never sees it. Each read is bounded by a
    /// timeout, not the whole transfer. Progress is whole percents;
    /// `cancelled` is polled per chunk.
    pub fn download_to(
        &self,
        url_or_path: &str,
        dest: &Path,
        on_progress: &dyn Fn(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, String> {
        let resp = self.open_stream(url_or_path).map_err(|e| match e {
            CanvasError::Unreachable(why) => format!("download failed: {why}"),
            other => other.to_string(),
        })?;
        if !(200..300).contains(&resp.status) {
            return Err(format!("download HTTP {}", resp.status));
        }
        // A login page where a file should be means the session lapsed.
        if resp
            .header("content-type")
            .is_some_and(|t| t.contains("text/html"))
        {
            return Err("got HTML instead of the file — session or URL problem".to_string());
        }
        let total = resp
            .header("content-length")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(0);

        let mut reader = resp.body;
        let mut file =
            std::fs::File::create(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = 0u64;
        let mut last_pct = u8::MAX;
        loop {
            if cancelled() {
                return Err(CANCELLED.to_string());
            }
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("network read failed after {done} bytes: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("write failed after {done} bytes: {e}"))?;
            done += n as u64;
            if total > 0 {
                let pct = (done * 100 / total).min(100) as u8;
                if pct != last_pct {
                    last_pct = pct;
                    on_progress(pct);
                }
            }
        }
        // oculus-keyd ends a body early, without an error, when Canvas drops
        // the connection; the announced length is the only way to tell.
        if total > 0 && done != total {
            return Err(format!("download incomplete: {done} of {total} bytes"));
        }
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(done)
    }

    /// The final response of the redirect chain from `url_or_path`, its body
    /// unread.
    fn open_stream(&self, url_or_path: &str) -> Result<Streamed, CanvasError> {
        let mut url = resolve(url_or_path)?;
        for followed in 0.. {
            let resp = if is_canvas(&url) {
                self.hop_stream(&url)?
            } else {
                transport::direct_stream(&url)?
            };
            let Some(next) = transport::redirect(resp.status, resp.header("location"), &url)?
            else {
                return Ok(resp);
            };
            if followed == transport::MAX_REDIRECTS {
                return Err(CanvasError::Failed(format!(
                    "the download redirected more than {} times",
                    transport::MAX_REDIRECTS
                )));
            }
            url = next;
        }
        unreachable!("the loop returns")
    }
}
