//! The PUT body that counts what was read.

use std::io::Read;
use std::time::{Duration, Instant};

/// A PUT body that counts what `ureq` has read, for the `uploading` phase,
/// and fails the next read once the file is skipped, which aborts the request.
pub(super) struct UploadBody<'a, R> {
    inner: R,
    sent: u64,
    total: u64,
    every: Duration,
    reported: Option<(u64, Instant)>,
    report: &'a dyn Fn(u64),
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a, R: Read> UploadBody<'a, R> {
    pub(super) fn new(
        inner: R,
        total: u64,
        every: Duration,
        report: &'a dyn Fn(u64),
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            inner,
            sent: 0,
            total,
            every,
            reported: None,
            report,
            cancelled,
        }
    }
}

impl<R: Read> Read for UploadBody<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if (self.cancelled)() {
            // Not `Interrupted`, which `io::copy` would retry.
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "upload skipped",
            ));
        }
        let read = self.inner.read(buf)?;
        self.sent += read as u64;
        let sent = self.sent.min(self.total);
        let finished = read == 0 || sent >= self.total;
        let due = match self.reported {
            None => true,
            Some((bytes, at)) => bytes != sent && (finished || at.elapsed() >= self.every),
        };
        if due {
            self.reported = Some((sent, Instant::now()));
            (self.report)(sent);
        }
        Ok(read)
    }
}
