//! Requests on the consumer: one is checked and saved in the step that
//! registers its route, so it is fully stored before any release of its
//! session or never stored for it; an answer's delivery is saved as the
//! bridge proves it; and a request whose session or turn ends is settled
//! here, the one place that does. docs/harness.md, "Requests".

use crate::harness::core::effect::{Effect, Reply as ReplyEffect};
use crate::harness::requests::{
    self, policy, reasons, Action, Delivery, LiveRoute, NewRequest, PermissionMode, Reply,
    RequestView, Status,
};
use crate::harness::{Generation, HarnessEvent, Provider, Release};

use super::super::run_scope::From;
use super::{Consumer, ConsumerIo, Stored};

impl Consumer {
    /// A bridge's CLI is waiting on the student. Declined at once, and
    /// never shown, when no turn is open to show it in, when the chat is
    /// saved as "Don't ask" now (whatever its turn started under), when it
    /// names a protected path, or when it cannot be saved.
    pub(super) fn request_asked(
        &mut self,
        thread_id: i64,
        provider: Provider,
        from: From,
        request: NewRequest,
        route: Option<LiveRoute>,
        io: &mut impl ConsumerIo,
    ) {
        let Some(route) = route else {
            elog!("[oculus] harness thread {thread_id}: a request with no way to answer it");
            return;
        };
        let From::Session(generation) = from else {
            return decline(thread_id, &request, "it came from no session", route, io);
        };
        if self.runs.open_run(thread_id).is_none() && !self.runs.own_open(thread_id) {
            return decline(thread_id, &request, "no turn is open", route, io);
        }
        if request.action == Action::Question
            && requests::QuestionPayload::read(&request.payload).is_err()
        {
            return decline(thread_id, &request, "invalid or secret question", route, io);
        }
        let library = io.library();
        let cwd = crate::agents::agents_dir(&library);
        if let Some(path) = policy::protected(&library, &cwd, &request.paths) {
            let why = format!("it names the protected path {path}");
            return decline(thread_id, &request, &why, route, io);
        }
        let Some(pool) = self.pool.clone() else {
            return io.perform(Effect::Withdraw {
                routes: vec![route],
            });
        };
        match self.rt.block_on(requests::store::mode(&pool, thread_id)) {
            Ok(PermissionMode::Quiet) if request.action != Action::Question => {
                return decline(
                    thread_id,
                    &request,
                    "the chat is set to Don't ask",
                    route,
                    io,
                );
            }
            Ok(_) => {}
            Err(e) => {
                let why = format!("its chat's mode could not be read: {e}");
                return decline(thread_id, &request, &why, route, io);
            }
        }
        self.flush_draft(thread_id, io.now());
        let asked = HarnessEvent::RequestAsked {
            request: request.clone(),
            route: None,
        };
        let owner = self.runs.owner(thread_id, &asked, None);
        let saved = requests::store::open(
            &pool,
            thread_id,
            owner.as_ref(),
            (provider, generation),
            &request,
        );
        let opened = match self.rt.block_on(saved) {
            Ok(opened) => opened,
            Err(e) => {
                elog!("[oculus] harness thread {thread_id}: a request could not be saved: {e}");
                return io.perform(Effect::Withdraw {
                    routes: vec![route],
                });
            }
        };
        self.revisions.insert(thread_id, opened.revision);
        if owner.is_some() {
            self.runs.written(thread_id);
        }
        self.requests
            .insert(opened.request.id, thread_id, generation, route);
        let waiting = self.waiting(thread_id);
        let stored = Stored {
            thread_id,
            provider,
            from,
            ev: HarnessEvent::RequestOpened {
                request: opened.request,
                waiting,
                waiting_questions: self.waiting_questions(thread_id),
            },
            card: None,
            tags: self.runs.tags(thread_id, None),
            item_id: Some(opened.item_id),
            entity_id: owner.map(|o| o.entity),
            terminal: None,
        };
        self.publish(stored, Vec::new(), io);
    }

    /// The provider retired a callback. One still waiting was withdrawn;
    /// one whose answer was on its way has arrived. A callback of no
    /// request saved here (declined unseen, or already settled) changes
    /// nothing.
    pub(super) fn request_retired(
        &mut self,
        thread_id: i64,
        provider: Provider,
        from: From,
        native_ref: &str,
        io: &mut impl ConsumerIo,
    ) {
        let (Some(pool), From::Session(generation)) = (self.pool.clone(), from) else {
            return;
        };
        let found = requests::store::by_native(&pool, thread_id, generation, native_ref);
        let request = match self.rt.block_on(found) {
            Ok(Some(request)) => request,
            Ok(None) => return,
            Err(e) => return elog!("[oculus] harness thread {thread_id}: request: {e}"),
        };
        self.requests.remove(request.id);
        let changed = match request.status {
            Status::Pending => self.rt.block_on(requests::store::end(
                &pool,
                thread_id,
                &[request.id],
                Status::Expired,
                reasons::WITHDRAWN,
            )),
            _ => self
                .rt
                .block_on(requests::store::delivered(
                    &pool,
                    request.id,
                    Delivery::Acknowledged,
                ))
                .map(|d| {
                    d.map(|(request, revision)| (vec![request], revision))
                        .unwrap_or_default()
                }),
        };
        self.tell_requests(thread_id, provider, changed, io);
    }

    /// The answer was committed: tell every view, and write it to the
    /// provider off this thread.
    pub(super) fn responded(
        &mut self,
        thread_id: i64,
        provider: Provider,
        request: RequestView,
        io: &mut impl ConsumerIo,
    ) {
        let message = request.responder == "message";
        let answers = request.answers.clone().unwrap_or_default();
        let option = request
            .options
            .iter()
            .find(|o| Some(&o.id) == request.response.as_ref())
            .cloned();
        let route = self.requests.answerable(request.id);
        let request_id = request.id;
        let waiting = self.waiting(thread_id);
        let updated = HarnessEvent::RequestUpdated {
            request,
            waiting,
            waiting_questions: self.waiting_questions(thread_id),
        };
        self.event(thread_id, provider, From::App, updated, None, io);
        if message {
            return;
        }
        match (route, option) {
            (Some(route), Some(option)) => {
                self.requests.sending(request_id, true);
                io.perform(Effect::Reply(Box::new(ReplyEffect {
                    thread_id,
                    provider,
                    request_id,
                    route,
                    option,
                    answers,
                })));
            }
            // The decider found the route a moment ago, on this thread.
            _ => self.request_replied(thread_id, provider, request_id, failed(), io),
        }
    }

    /// What the reply's write proved, saved and told.
    pub(super) fn request_replied(
        &mut self,
        thread_id: i64,
        provider: Provider,
        request_id: i64,
        reply: Reply,
        io: &mut impl ConsumerIo,
    ) {
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let delivery = match &reply {
            // The provider will retire the callback; until it does, the
            // route stays, so a release can say the answer is uncertain.
            Reply::Written => return,
            Reply::Acknowledged => Delivery::Acknowledged,
            Reply::Failed(_) => Delivery::Failed,
            Reply::Uncertain(_) => Delivery::Uncertain,
        };
        if let Reply::Failed(e) | Reply::Uncertain(e) = &reply {
            elog!("[oculus] harness request {request_id}: the answer's write: {e}");
        }
        match delivery {
            // Nothing arrived: the student may answer again.
            Delivery::Failed => self.requests.sending(request_id, false),
            _ => {
                self.requests.remove(request_id);
            }
        }
        let saved = requests::store::delivered(&pool, request_id, delivery);
        let changed = self.rt.block_on(saved).map(|d| {
            d.map(|(request, revision)| (vec![request], revision))
                .unwrap_or_default()
        });
        self.tell_requests(thread_id, provider, changed, io);
    }

    /// The session `generation` is over: its requests can no longer be
    /// answered. A deliberate end cancels them and tells a live CLI no; a
    /// process that died expires them; a quit leaves them to startup.
    pub(super) fn end_requests(
        &mut self,
        thread_id: i64,
        provider: Provider,
        generation: Generation,
        reason: Release,
        io: &mut impl ConsumerIo,
    ) {
        let routes = self.requests.release(thread_id, generation);
        if routes.is_empty() {
            return;
        }
        let (status, why) = match reason {
            Release::Stopped => (Status::Cancelled, reasons::STOPPED),
            Release::Replaced => (Status::Cancelled, reasons::REPLACED),
            Release::Exited => (Status::Expired, reasons::PROCESS_ENDED),
            Release::Deleted | Release::Quit => (Status::Cancelled, ""),
        };
        let ids: Vec<i64> = routes.iter().map(|(id, _)| *id).collect();
        if matches!(
            reason,
            Release::Stopped | Release::Replaced | Release::Deleted
        ) {
            let routes: Vec<LiveRoute> = routes.into_iter().filter_map(|(_, r)| r).collect();
            if !routes.is_empty() {
                io.perform(Effect::Withdraw { routes });
            }
        }
        if matches!(reason, Release::Deleted | Release::Quit) {
            return;
        }
        let Some(pool) = self.pool.clone() else {
            return;
        };
        let changed = self
            .rt
            .block_on(requests::store::end(&pool, thread_id, &ids, status, why));
        self.tell_requests(thread_id, provider, changed, io);
    }

    /// A run's turn finished with requests still waiting: they belonged to
    /// it, so they end with it. After an interrupt the CLI retires their
    /// callbacks itself; a turn that ended otherwise may leave one still
    /// listening (a spawned child's), which is told no.
    pub(super) fn requests_left_by(
        &mut self,
        thread_id: i64,
        run_id: Option<i64>,
        status: &str,
    ) -> Vec<(HarnessEvent, Option<i64>)> {
        let Some(pool) = self.pool.clone() else {
            return Vec::new();
        };
        let left = self
            .rt
            .block_on(requests::store::left_by(&pool, thread_id, run_id));
        let mut ids = match left {
            Ok(ids) if !ids.is_empty() => ids,
            Ok(_) => return Vec::new(),
            Err(e) => {
                elog!("[oculus] harness thread {thread_id}: requests: {e}");
                return Vec::new();
            }
        };
        if status == "failed" {
            ids.retain(|id| {
                self.rt
                    .block_on(async {
                        let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
                        requests::store::get(&mut conn, *id).await
                    })
                    .ok()
                    .flatten()
                    .is_none_or(|r| r.kind != "question")
            });
        }
        let routes = ids.iter().filter_map(|id| self.requests.remove(*id));
        let routes: Vec<LiveRoute> = routes.collect();
        if status != "interrupted" {
            self.withdrawals.extend(routes);
        }
        let why = match status {
            "interrupted" => reasons::STOPPED,
            _ => reasons::TURN_ENDED,
        };
        let ended = requests::store::end(&pool, thread_id, &ids, Status::Cancelled, why);
        match self.rt.block_on(ended) {
            Ok((changed, revision)) => {
                if revision > 0 {
                    self.revisions.insert(thread_id, revision);
                }
                let waiting = self.waiting(thread_id);
                changed
                    .into_iter()
                    .map(|request| {
                        (
                            HarnessEvent::RequestUpdated {
                                request,
                                waiting,
                                waiting_questions: self.waiting_questions(thread_id),
                            },
                            None,
                        )
                    })
                    .collect()
            }
            Err(e) => {
                elog!("[oculus] harness thread {thread_id}: requests: {e}");
                Vec::new()
            }
        }
    }

    /// Tell no to the callbacks a step gave up where it had no way to.
    pub(super) fn withdraw_left(&mut self, io: &mut impl ConsumerIo) {
        if !self.withdrawals.is_empty() {
            let routes = std::mem::take(&mut self.withdrawals);
            io.perform(Effect::Withdraw { routes });
        }
    }

    /// The thread is gone: its routes are told no and dropped.
    pub(super) fn forget_requests(&mut self, thread_id: i64, io: &mut impl ConsumerIo) {
        let routes = self.requests.forget(thread_id);
        if !routes.is_empty() {
            io.perform(Effect::Withdraw { routes });
        }
    }

    fn tell_requests(
        &mut self,
        thread_id: i64,
        provider: Provider,
        changed: Result<(Vec<RequestView>, i64), String>,
        io: &mut impl ConsumerIo,
    ) {
        match changed {
            Ok((requests, revision)) => {
                if revision > 0 {
                    self.revisions.insert(thread_id, revision);
                }
                let waiting = self.waiting(thread_id);
                for request in requests {
                    let updated = HarnessEvent::RequestUpdated {
                        request,
                        waiting,
                        waiting_questions: self.waiting_questions(thread_id),
                    };
                    self.event(thread_id, provider, From::App, updated, None, io);
                }
            }
            Err(e) => elog!("[oculus] harness thread {thread_id}: requests: {e}"),
        }
    }

    /// The thread's questions waiting on the student, read after a change
    /// is saved; 0 when it cannot be read.
    fn waiting_questions(&self, thread_id: i64) -> u32 {
        let Some(pool) = &self.pool else {
            return 0;
        };
        self.rt
            .block_on(requests::store::waiting_questions(pool, thread_id))
            .unwrap_or_default()
    }

    /// The thread's requests waiting on the student, read after a change
    /// is saved; 0 when it cannot be read.
    fn waiting(&self, thread_id: i64) -> u32 {
        let Some(pool) = self.pool.as_ref() else {
            return 0;
        };
        match self.rt.block_on(requests::store::waiting(pool, thread_id)) {
            Ok(n) => n,
            Err(e) => {
                elog!("[oculus] harness thread {thread_id}: waiting requests: {e}");
                0
            }
        }
    }
}

fn failed() -> Reply {
    Reply::Failed("the request's route was gone".into())
}

/// Tell the CLI no without showing the request: it goes no further than a
/// log line, and the agent explains.
fn decline(
    thread_id: i64,
    request: &NewRequest,
    why: &str,
    route: LiveRoute,
    io: &mut impl ConsumerIo,
) {
    elog!(
        "[oculus] harness thread {thread_id}: declined a {} request: {why}",
        request.action.as_str()
    );
    io.perform(Effect::Withdraw {
        routes: vec![route],
    });
}
