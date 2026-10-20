//! The authority's in-process events, for every share of the file: a change
//! goes into the share's log and out to its sockets; a pause or resume is
//! pushed as `state`; the session ending revokes the share. This runs under
//! the session's lock (authority/events.rs), so it only touches the grants
//! and leaves anything slower to `Core::end_file`'s thread.

use crate::companion::core::Core;
use crate::library::files::canvases::authority::events::{Event, Phase};
use std::sync::Arc;

pub fn on_event(core: &Arc<Core>, event: &Event) {
    match event {
        Event::Change(change) => {
            if let Some(share) = core.grants.by_file(change.file_id) {
                share.record(change);
            }
        }
        Event::State(state) => {
            let Some(share) = core.grants.by_file(state.file_id) else {
                return;
            };
            // A share still reading its base has no epoch yet and takes the event.
            if share.epoch().is_some_and(|epoch| epoch != state.epoch) {
                return;
            }
            match state.phase {
                Phase::Ended => core.end_file(state.file_id),
                phase => {
                    share.set_phase(phase);
                    core.notify();
                }
            }
        }
    }
}
