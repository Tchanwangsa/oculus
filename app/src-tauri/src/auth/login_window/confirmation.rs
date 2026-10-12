//! Coalesces completed Canvas loads while the saved session is being checked.

#[derive(Default)]
pub(super) struct Confirmation {
    running: bool,
    pending: bool,
    resolved: bool,
    closed: bool,
}

impl Confirmation {
    pub fn request(&mut self) -> bool {
        if self.closed || self.resolved {
            return false;
        }
        if self.running {
            self.pending = true;
            return false;
        }
        self.running = true;
        true
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn resolve(&mut self, announce: impl FnOnce()) -> bool {
        if self.closed {
            return false;
        }
        self.resolved = true;
        self.running = false;
        announce();
        true
    }

    pub fn retry(&mut self) -> bool {
        if !self.closed && !self.resolved && std::mem::take(&mut self.pending) {
            return true;
        }
        self.running = false;
        false
    }

    /// Whether closing this window cancels a sign-in still being confirmed.
    pub fn close(&mut self) -> bool {
        self.closed = true;
        !self.resolved
    }
}

#[cfg(test)]
mod tests {
    use super::Confirmation;

    #[test]
    fn dashboard_load_during_an_anonymous_probe_gets_another_capture() {
        let mut state = Confirmation::default();
        assert!(state.request());
        assert!(!state.request());
        assert!(!state.request());
        assert!(state.retry());
        assert!(!state.retry());
        assert!(state.request());
        assert!(state.resolve(|| {}));
        assert!(!state.request());
        assert!(!state.close());
    }

    #[test]
    fn closing_during_a_probe_cancels_even_with_a_pending_load() {
        let mut state = Confirmation::default();
        assert!(state.request());
        assert!(!state.request());
        assert!(state.close());
        assert!(state.closed());
        assert!(!state.retry());
        assert!(!state.request());
        assert!(!state.resolve(|| panic!("a closed window cannot announce success")));
    }
}
