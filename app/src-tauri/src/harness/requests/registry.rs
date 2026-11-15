//! The live routes of saved requests, by request id. The consumer owns it:
//! a route goes in in the step that saves its request, and comes out when
//! the request is settled or its session is released, so a route here is
//! one the student can still answer through.

use std::collections::HashMap;

use crate::harness::Generation;

use super::LiveRoute;

struct Entry {
    thread_id: i64,
    generation: Generation,
    route: LiveRoute,
    /// An answer was written and the provider has not confirmed it yet.
    sending: bool,
}

#[derive(Default)]
pub(in crate::harness) struct Registry {
    routes: HashMap<i64, Entry>,
}

impl Registry {
    pub fn insert(
        &mut self,
        request_id: i64,
        thread_id: i64,
        generation: Generation,
        route: LiveRoute,
    ) {
        let entry = Entry {
            thread_id,
            generation,
            route,
            sending: false,
        };
        self.routes.insert(request_id, entry);
    }

    /// The route of a request still waiting for its answer.
    pub fn answerable(&self, request_id: i64) -> Option<LiveRoute> {
        self.routes
            .get(&request_id)
            .filter(|e| !e.sending)
            .map(|e| e.route.clone())
    }

    /// An answer is on its way: the route stays until it is confirmed.
    pub fn sending(&mut self, request_id: i64, sending: bool) {
        if let Some(entry) = self.routes.get_mut(&request_id) {
            entry.sending = sending;
        }
    }

    pub fn remove(&mut self, request_id: i64) -> Option<LiveRoute> {
        self.routes.remove(&request_id).map(|e| e.route)
    }

    /// Every request of the session `generation` on this thread, taken out:
    /// its session is over.
    pub fn release(&mut self, thread_id: i64, generation: Generation) -> Vec<(i64, LiveRoute)> {
        let ids: Vec<i64> = self
            .routes
            .iter()
            .filter(|(_, e)| e.thread_id == thread_id && e.generation == generation)
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter()
            .filter_map(|id| self.routes.remove(&id).map(|e| (id, e.route)))
            .collect()
    }

    /// The thread is gone, and every route with it.
    pub fn forget(&mut self, thread_id: i64) -> Vec<LiveRoute> {
        let ids: Vec<i64> = self
            .routes
            .iter()
            .filter(|(_, e)| e.thread_id == thread_id)
            .map(|(id, _)| *id)
            .collect();
        ids.into_iter().filter_map(|id| self.remove(id)).collect()
    }
}
