/// Who asked for a sign-in. `Manual` is a person waiting on the answer, so it
/// skips the 10 min to 6 h wait between automatic attempts; every trigger
/// still keeps `MIN_SPACING` from the last attempt (`guard::admit`). `Forward`
/// is keyd's own, for a rejected Canvas request, and is automatic like the
/// rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Manual,
    Startup,
    Browser,
    Forward,
}

pub(super) const ALL: [Trigger; 4] = [
    Trigger::Manual,
    Trigger::Startup,
    Trigger::Browser,
    Trigger::Forward,
];

impl Trigger {
    /// The name a client sends keyd, and keyd's reply log uses.
    pub fn wire_name(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Startup => "startup",
            Trigger::Browser => "browser",
            Trigger::Forward => "forward",
        }
    }

    pub fn from_wire_name(name: &str) -> Option<Trigger> {
        ALL.into_iter().find(|t| t.wire_name() == name)
    }

    /// The label in `okta-sign-in.log`.
    pub(in crate::okta) fn as_str(self) -> &'static str {
        match self {
            Trigger::Manual => "manual",
            Trigger::Startup => "app startup",
            Trigger::Browser => "browser",
            Trigger::Forward => "forward",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trigger_has_a_wire_name_that_reads_back() {
        for trigger in ALL {
            assert_eq!(Trigger::from_wire_name(trigger.wire_name()), Some(trigger));
        }
        assert_eq!(Trigger::Forward.wire_name(), "forward");
        assert_eq!(Trigger::Forward.as_str(), "forward");
        assert_eq!(Trigger::from_wire_name("Forward"), None);
        assert_eq!(Trigger::from_wire_name("tick"), None);
    }
}
