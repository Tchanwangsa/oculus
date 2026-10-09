/// Who asked for a sign-in. `Manual` is a person waiting on the answer, so it
/// skips the 10 min to 6 h wait between automatic attempts; every trigger
/// still keeps `MIN_SPACING` from the last attempt (`guard::admit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Startup,
    KeepAlive,
    Browser,
}

impl Trigger {
    /// The name a client sends keyd, and keyd's reply log uses.
    pub fn wire_name(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Startup => "startup",
            Trigger::KeepAlive => "keep-alive",
            Trigger::Browser => "browser",
        }
    }

    pub fn from_wire_name(name: &str) -> Option<Trigger> {
        [
            Trigger::Manual,
            Trigger::Startup,
            Trigger::KeepAlive,
            Trigger::Browser,
        ]
        .into_iter()
        .find(|t| t.wire_name() == name)
    }

    /// The label in `okta-sign-in.log`.
    pub(in crate::okta) fn as_str(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Startup => "app startup",
            Trigger::KeepAlive => "keep-alive",
            Trigger::Browser => "browser",
        }
    }
}
