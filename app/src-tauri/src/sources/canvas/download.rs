use std::path::Path;

use super::transport::{self, is_canvas, resolve, Streamed};
use super::{Canvas, CanvasError};
use crate::runtime::stream::{copy_to_file, percent};

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
            .and_then(|s| s.trim().parse::<u64>().ok());

        let last_pct = std::cell::Cell::new(u8::MAX);
        let mut body = resp.body;
        copy_to_file(
            &mut body,
            dest,
            total,
            &|done| {
                if let Some(total) = total.filter(|&t| t > 0) {
                    let pct = percent(done, total);
                    if pct != last_pct.replace(pct) {
                        on_progress(pct);
                    }
                }
            },
            cancelled,
        )
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
