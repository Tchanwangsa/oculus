//! What a run reports while it works, and the waits behind a stalled row.

use serde::Serialize;

/// Reported while a run works through a document; `total_pages` may be zero
/// until the backend knows. Same shape as `parse::Progress` so the sidebar
/// renders both with one component.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Progress {
    pub pages_done: u32,
    pub total_pages: u32,
    pub backend: &'static str,
    /// Set while every request in flight is held back by a rate limit, so a
    /// row that is not moving can say why and for how long.
    pub waiting: Option<Wait>,
}

/// A request held back by a rate limit, and when it is expected to go out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Wait {
    /// Epoch milliseconds.
    pub until_ms: u64,
    pub limiter: Limiter,
}

impl Wait {
    pub fn after(duration: std::time::Duration, limiter: Limiter) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            until_ms: (now + duration).as_millis() as u64,
            limiter,
        }
    }
}

/// Which limit holds the request: the server's refusal, or our own pacing to
/// one of the account's per-minute ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Limiter {
    /// Voyage answered 429.
    Throttled,
    Requests {
        per_minute: u32,
    },
    Tokens {
        per_minute: u32,
    },
}

impl Limiter {
    /// A lower-case phrase for the pipeline row, e.g. "pacing to Voyage's
    /// 3 requests/min limit".
    pub fn describe(&self) -> String {
        match self {
            Limiter::Throttled => "rate-limited by Voyage".to_string(),
            Limiter::Requests { per_minute } => {
                format!(
                    "pacing to Voyage's {} requests/min limit",
                    compact(*per_minute)
                )
            }
            Limiter::Tokens { per_minute } => {
                format!(
                    "pacing to Voyage's {} tokens/min limit",
                    compact(*per_minute)
                )
            }
        }
    }
}

/// 3 -> "3", 10000 -> "10K", 2000000 -> "2M".
fn compact(n: u32) -> String {
    match n {
        n if n >= 1_000_000 && n % 1_000_000 == 0 => format!("{}M", n / 1_000_000),
        n if n >= 1_000 && n % 1_000 == 0 => format!("{}K", n / 1_000),
        n => n.to_string(),
    }
}
