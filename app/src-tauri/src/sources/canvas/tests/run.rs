//! A sync run against a Canvas session that has gone.

use std::sync::{Arc, Mutex};

use keyd_core::okta::LoginError;

use crate::sync::{Engine, Progress, Reporter, Subject};

use super::{answer, refused, rig};

#[derive(Default)]
struct Seen {
    logs: Mutex<Vec<(String, String, String)>>,
    expired: Mutex<Vec<String>>,
    progress: Mutex<Vec<(String, String)>>,
}

struct Spy(Arc<Seen>);

impl Reporter for Spy {
    fn progress(&self, p: &Progress) {
        self.0
            .progress
            .lock()
            .unwrap()
            .push((p.course.clone(), p.phase.clone()));
    }
    fn log(&self, level: &str, course: &str, message: &str) {
        self.0
            .logs
            .lock()
            .unwrap()
            .push((level.into(), course.into(), message.into()));
    }
    fn canvas_expired(&self, message: &str) {
        self.0.expired.lock().unwrap().push(message.into());
    }
}

fn subjects() -> Vec<Subject> {
    ["COMP1", "COMP2", "COMP3"]
        .iter()
        .enumerate()
        .map(|(i, code)| Subject {
            id: i as i64 + 1,
            code: code.to_string(),
        })
        .collect()
}

fn errors(seen: &Seen) -> Vec<(String, String, String)> {
    seen.logs
        .lock()
        .unwrap()
        .iter()
        .filter(|(level, ..)| level == "error")
        .cloned()
        .collect()
}

#[test]
fn a_sync_stops_at_the_first_dead_session_and_says_so_once() {
    let r = rig(|_, _| refused(401, &[], b"{}", LoginError::Paused("a bad password".into())));
    let seen = Arc::new(Seen::default());
    let engine = Engine::new(&r.dir, Box::new(Spy(seen.clone())));

    let attempted = engine.scrape(&subjects());

    assert_eq!(attempted, 1);
    let expired = seen.expired.lock().unwrap().clone();
    assert_eq!(expired.len(), 1, "the event fires once");
    assert!(expired[0].contains("COMP1"), "{}", expired[0]);
    assert!(
        expired[0].contains("Automatic sign-in is paused"),
        "{}",
        expired[0]
    );
    let errors = errors(&seen);
    assert_eq!(errors.len(), 1, "one error, not one per course: {errors:?}");
    assert_eq!(errors[0].2, expired[0]);
    let courses: Vec<String> = seen
        .progress
        .lock()
        .unwrap()
        .iter()
        .map(|(course, _)| course.clone())
        .collect();
    assert!(!courses.contains(&"COMP2".to_string()), "{courses:?}");
    assert_eq!(r.forwards().len(), 1, "later requests never reach keyd");
}

#[test]
fn a_failing_course_that_is_not_an_expiry_does_not_stop_the_run() {
    let r = rig(|_, _| answer(503, &[], b"down"));
    let seen = Arc::new(Seen::default());
    let engine = Engine::new(&r.dir, Box::new(Spy(seen.clone())));

    let attempted = engine.scrape(&subjects());

    assert_eq!(attempted, 3);
    assert!(seen.expired.lock().unwrap().is_empty());
    let errors = errors(&seen);
    assert_eq!(errors.len(), 3, "{errors:?}");
    assert_eq!(
        errors.iter().map(|e| e.1.as_str()).collect::<Vec<_>>(),
        ["COMP1", "COMP2", "COMP3"]
    );
}
