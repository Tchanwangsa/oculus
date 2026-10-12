//! The master key's one fetch per process. WebKit asks on every wrap and
//! unwrap, so the first ask starts the fetch, asks during it wait behind it,
//! and every later ask is answered from what it returned, `None` included.
//! Plain Rust over any handler type, so the delegate's glue stays thin.

pub(super) type Key = Option<[u8; 16]>;

/// What to do with one ask.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Ask<H> {
    /// Answer the handler now.
    Answer(Key, H),
    /// The handler is queued and nothing fetches yet: start the fetch.
    Fetch,
    /// The handler is queued behind the fetch in flight.
    Wait,
}

enum State<H> {
    Unasked,
    Fetching(Vec<H>),
    Known(Key),
}

pub(super) struct KeyCache<H> {
    state: State<H>,
}

impl<H> KeyCache<H> {
    pub(super) const fn new() -> Self {
        Self {
            state: State::Unasked,
        }
    }

    pub(super) fn ask(&mut self, handler: H) -> Ask<H> {
        match &mut self.state {
            State::Known(key) => Ask::Answer(*key, handler),
            State::Fetching(waiting) => {
                waiting.push(handler);
                Ask::Wait
            }
            State::Unasked => {
                self.state = State::Fetching(vec![handler]);
                Ask::Fetch
            }
        }
    }

    /// Records the fetch's result and hands back every waiting handler, in
    /// the order they asked, each to be answered once with `key`. A known
    /// key is never replaced.
    pub(super) fn settle(&mut self, key: Key) -> Vec<H> {
        match std::mem::replace(&mut self.state, State::Known(key)) {
            State::Fetching(waiting) => waiting,
            State::Unasked => Vec::new(),
            State::Known(known) => {
                self.state = State::Known(known);
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 16] = *b"foobarfoobarfoob";

    #[test]
    fn the_first_ask_fetches_and_the_rest_wait_for_it() {
        let mut cache = KeyCache::new();
        assert_eq!(cache.ask(1), Ask::Fetch);
        assert_eq!(cache.ask(2), Ask::Wait);
        assert_eq!(cache.ask(3), Ask::Wait);
        assert_eq!(cache.settle(Some(KEY)), vec![1, 2, 3]);
        assert_eq!(cache.ask(4), Ask::Answer(Some(KEY), 4));
        assert_eq!(cache.ask(5), Ask::Answer(Some(KEY), 5));
    }

    #[test]
    fn no_key_is_kept_too_so_keyd_is_asked_once() {
        let mut cache = KeyCache::new();
        assert_eq!(cache.ask(1), Ask::Fetch);
        assert_eq!(cache.settle(None), vec![1]);
        assert_eq!(cache.ask(2), Ask::Answer(None, 2));
    }

    #[test]
    fn a_second_settle_answers_no_one_and_keeps_the_first_key() {
        let mut cache = KeyCache::new();
        assert_eq!(cache.ask(1), Ask::Fetch);
        assert_eq!(cache.settle(Some(KEY)), vec![1]);
        assert_eq!(cache.settle(None), Vec::<i32>::new());
        assert_eq!(cache.ask(2), Ask::Answer(Some(KEY), 2));
    }
}
