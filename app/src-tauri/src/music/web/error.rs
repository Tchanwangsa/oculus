//! How a Web API call fails, worded for the page that shows it.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::music) enum WebError {
    NotConnected,
    /// Login5 would not hand out a token.
    Token(String),
    /// A 401. The caller asks for a new token once before giving up.
    Unauthorized,
    /// A 401 again with a new token, or Login5 handed back the refused one.
    StillUnauthorized,
    Forbidden,
    /// A 429, or the gate it closed: seconds until calls may resume.
    RateLimited(u64),
    Status(u16),
    Transport(String),
    Unreadable(String),
}

impl fmt::Display for WebError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => write!(f, "Not connected to Spotify"),
            Self::Token(why) => write!(f, "Spotify would not give Oculus a Web API token ({why})"),
            Self::Unauthorized | Self::StillUnauthorized => {
                write!(f, "Spotify refused the Web API token (401)")
            }
            Self::Forbidden => write!(f, "Spotify refused the request (403)"),
            Self::RateLimited(secs) => write!(
                f,
                "Spotify is rate-limiting Oculus; try again in {} s",
                secs.max(&1)
            ),
            Self::Status(code) => write!(f, "Spotify's Web API answered {code}"),
            Self::Transport(why) => write!(f, "Could not reach Spotify's Web API ({why})"),
            Self::Unreadable(why) => {
                write!(f, "Spotify's Web API sent something unreadable ({why})")
            }
        }
    }
}

impl From<WebError> for String {
    fn from(e: WebError) -> Self {
        e.to_string()
    }
}
