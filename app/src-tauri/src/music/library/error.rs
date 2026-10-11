//! How a library call fails, worded for the page that shows it.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::music) enum LibraryError {
    NotConnected,
    /// Login5 would not hand out the session's access token.
    Token(String),
    /// A 401: Spotify refused the session's token.
    Unauthorized,
    Forbidden,
    /// A 429, or the gate it closed: seconds until calls may resume.
    RateLimited(u64),
    /// Search answered with a status that means the route is gone or
    /// refused for this client, not that the query was bad.
    SearchUnavailable(u16),
    Status(u16),
    Transport(String),
    Unreadable(String),
}

impl LibraryError {
    /// An answer asking again would not change — a refusal, another status,
    /// a body that will not decode — as against a rate limit, a token or the
    /// connection.
    pub(super) fn is_lasting(&self) -> bool {
        matches!(
            self,
            Self::Forbidden | Self::Status(_) | Self::Unreadable(_)
        )
    }
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConnected => write!(f, "Not connected to Spotify"),
            Self::Token(why) => write!(f, "Spotify would not give Oculus a token ({why})"),
            Self::Unauthorized => write!(f, "Spotify refused Oculus's token (401)"),
            Self::Forbidden => write!(f, "Spotify refused the request (403)"),
            Self::RateLimited(secs) => write!(
                f,
                "Spotify is rate-limiting Oculus; try again in {} s",
                secs.max(&1)
            ),
            Self::SearchUnavailable(code) => {
                write!(
                    f,
                    "Search isn't available right now (Spotify answered {code})"
                )
            }
            Self::Status(code) => write!(f, "Spotify answered {code}"),
            Self::Transport(why) => write!(f, "Could not reach Spotify ({why})"),
            Self::Unreadable(why) => write!(f, "Spotify sent something unreadable ({why})"),
        }
    }
}

impl From<LibraryError> for String {
    fn from(e: LibraryError) -> Self {
        e.to_string()
    }
}
