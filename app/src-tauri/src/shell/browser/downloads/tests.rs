use std::path::{Path, PathBuf};

use super::state::{Effect, Target};
use super::{sweep_incoming, Dest, Downloads, SavedTo, State};
use crate::library::files::uploads::ImportOutcome;

const ROOT: &str = "/data/downloads/.incoming";
const URL: &str = "https://files.example.com/courses/1/files/2/download?verifier=SECRET";

fn url(s: &str) -> url::Url {
    s.parse().unwrap()
}

/// One download of `URL` in tab 7.
fn started() -> (Downloads, u32, PathBuf) {
    let mut d = Downloads::default();
    let (id, staged) = d.request(7, 7, &url(URL), "notes.pdf".into(), Path::new(ROOT));
    (d, id, staged)
}

fn finish(d: &mut Downloads, success: bool) -> Option<(u32, Vec<Effect>)> {
    d.finish(URL, None, success, |_| Some(42))
}

fn state(d: &Downloads, id: u32) -> State {
    d.snapshot()
        .downloads
        .iter()
        .find(|r| r.id == id)
        .unwrap()
        .state
}

fn oculus() -> Dest {
    Dest::Oculus {
        subject_code: "COMP30027".into(),
    }
}

fn deliveries(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::Deliver { .. }))
        .count()
}

#[test]
fn each_download_is_staged_in_its_own_folder() {
    let (mut d, id, staged) = started();
    assert_eq!(
        staged,
        Path::new(ROOT)
            .join(format!("{}-{id}", std::process::id()))
            .join("notes.pdf")
    );
    let (second, other) = d.request(7, 7, &url(URL), "notes.pdf".into(), Path::new(ROOT));
    assert_ne!(second, id);
    assert_ne!(other.parent(), staged.parent());
}

#[test]
fn a_choice_before_the_finish_delivers_once_at_the_finish() {
    let (mut d, id, staged) = started();
    assert!(
        d.choose(id, oculus()).unwrap().is_empty(),
        "still receiving"
    );
    assert_eq!(state(&d, id), State::Receiving);
    assert_eq!(d.snapshot().downloads[0].choice, Some(oculus()));

    let (found, effects) = finish(&mut d, true).unwrap();
    assert_eq!(found, id);
    assert_eq!(
        effects,
        [Effect::Deliver {
            id,
            staged,
            to: Target::Oculus {
                subject_code: "COMP30027".into()
            }
        }]
    );
    assert_eq!(state(&d, id), State::Delivering);
    assert_eq!(d.snapshot().downloads[0].received, 42, "the final size");

    assert!(d.choose(id, oculus()).is_err(), "no second delivery");
    assert!(
        finish(&mut d, true).is_none(),
        "a second finish matches nothing"
    );
}

#[test]
fn a_finish_before_the_choice_waits_then_delivers_once() {
    let (mut d, id, staged) = started();
    let (_, effects) = finish(&mut d, true).unwrap();
    assert!(effects.is_empty());
    assert_eq!(state(&d, id), State::Ready);

    let path = "/Users/student/Downloads/notes.pdf";
    let effects = d.choose(id, Dest::Path { path: path.into() }).unwrap();
    assert_eq!(
        effects,
        [Effect::Deliver {
            id,
            staged,
            to: Target::Path {
                path: path.into(),
                may_replace: false
            }
        }]
    );
    assert!(d.choose(id, oculus()).is_err());
    assert!(d.choose(id, Dest::Discard).is_err());
}

#[test]
fn only_the_path_the_save_panel_confirmed_may_replace_a_file() {
    let (mut d, id, _) = started();
    finish(&mut d, true).unwrap();
    let confirmed = PathBuf::from("/Users/student/Desktop/notes.pdf");
    d.confirm(id, confirmed.clone());
    let to = |effects: Vec<Effect>| match effects.into_iter().next() {
        Some(Effect::Deliver { to, .. }) => to,
        other => panic!("{other:?}"),
    };
    let elsewhere = d
        .choose(
            id,
            Dest::Path {
                path: "/Users/student/Desktop/other.pdf".into(),
            },
        )
        .unwrap();
    assert_eq!(
        to(elsewhere),
        Target::Path {
            path: "/Users/student/Desktop/other.pdf".into(),
            may_replace: false
        }
    );
    d.delivered(id, Err("other.pdf already exists there".into()));
    let effects = d
        .choose(
            id,
            Dest::Path {
                path: confirmed.to_string_lossy().into(),
            },
        )
        .unwrap();
    assert_eq!(
        to(effects),
        Target::Path {
            path: confirmed,
            may_replace: true
        }
    );
}

#[test]
fn either_order_delivers_exactly_once_across_every_interleaving() {
    for choose_first in [true, false] {
        let (mut d, id, _) = started();
        let mut effects = vec![];
        if choose_first {
            effects.extend(d.choose(id, oculus()).unwrap());
            effects.extend(finish(&mut d, true).unwrap().1);
        } else {
            effects.extend(finish(&mut d, true).unwrap().1);
            effects.extend(d.choose(id, oculus()).unwrap());
        }
        let _ = d.choose(id, oculus());
        let _ = finish(&mut d, true);
        assert_eq!(deliveries(&effects), 1, "choose first: {choose_first}");
    }
}

#[test]
fn a_choice_can_change_while_bytes_arrive() {
    let (mut d, id, _) = started();
    d.choose(id, oculus()).unwrap();
    d.choose(
        id,
        Dest::Path {
            path: "/tmp/notes.pdf".into(),
        },
    )
    .unwrap();
    let (_, effects) = finish(&mut d, true).unwrap();
    assert!(matches!(
        &effects[..],
        [Effect::Deliver {
            to: Target::Path { .. },
            ..
        }]
    ));
}

#[test]
fn discarding_while_receiving_deletes_at_the_finish() {
    let (mut d, id, staged) = started();
    assert!(d.choose(id, Dest::Discard).unwrap().is_empty());
    assert_eq!(state(&d, id), State::Discarded);
    assert!(d.unfinished_in(7), "its page must still hear the end");

    let (_, effects) = finish(&mut d, true).unwrap();
    assert_eq!(effects, [Effect::Remove(staged)]);
    assert_eq!(state(&d, id), State::Discarded);
    assert!(!d.unfinished_in(7));
}

#[test]
fn discarding_a_ready_download_deletes_at_once() {
    let (mut d, id, staged) = started();
    finish(&mut d, true).unwrap();
    assert_eq!(
        d.choose(id, Dest::Discard).unwrap(),
        [Effect::Remove(staged)]
    );
    assert_eq!(state(&d, id), State::Discarded);
}

#[test]
fn a_failed_download_keeps_what_retry_needs_and_no_bytes() {
    let (mut d, id, staged) = started();
    d.choose(id, oculus()).unwrap();
    let (_, effects) = finish(&mut d, false).unwrap();
    assert_eq!(effects, [Effect::Remove(staged)], "no delivery, bytes go");
    let row = &d.snapshot().downloads[0];
    assert_eq!(row.state, State::Failed);
    assert!(row.error.is_some());
    assert!(row.can_retry);
    assert_eq!(d.retry_target(id).unwrap(), (7, url(URL)));
    assert!(d.choose(id, oculus()).is_err());

    let mut blob = Downloads::default();
    let (id, _) = blob.request(
        1,
        1,
        &url("blob:https://app.example.com/0b1c"),
        "export.csv".into(),
        Path::new(ROOT),
    );
    blob.finish("blob:https://app.example.com/0b1c", None, false, |_| None);
    assert!(!blob.snapshot().downloads[0].can_retry);
    assert!(blob.retry_target(id).is_err());
}

#[test]
fn a_delivery_that_does_not_land_is_ready_for_another_choice() {
    let (mut d, id, _) = started();
    finish(&mut d, true).unwrap();
    d.choose(id, oculus()).unwrap();
    d.delivered(id, Err("disk full".into()));
    let row = &d.snapshot().downloads[0];
    assert_eq!(row.state, State::Ready);
    assert_eq!(row.error.as_deref(), Some("disk full"));
    assert_eq!(row.choice, None);
    assert_eq!(deliveries(&d.choose(id, oculus()).unwrap()), 1);

    d.delivered(
        id,
        Ok(SavedTo::Path {
            path: "/tmp/notes.pdf".into(),
        }),
    );
    assert_eq!(state(&d, id), State::Saved);
    assert_eq!(
        d.saved_path(id, Path::new("/data")).unwrap(),
        PathBuf::from("/tmp/notes.pdf")
    );
}

#[test]
fn a_saved_upload_resolves_to_the_library_file() {
    let (mut d, id, _) = started();
    finish(&mut d, true).unwrap();
    d.choose(id, oculus()).unwrap();
    assert!(
        d.saved_path(id, Path::new("/data")).is_err(),
        "not saved yet"
    );
    let outcome = ImportOutcome {
        source: "notes.pdf".into(),
        file: Some(crate::library::files::ImportedFile {
            filename: "notes.pdf".into(),
            relative_path: "courses/COMP30027/uploads/notes.pdf".into(),
            file_type: "pdf".into(),
            size_bytes: 42,
        }),
        error: None,
    };
    d.delivered(
        id,
        Ok(SavedTo::Oculus {
            subject_code: "COMP30027".into(),
            outcome,
        }),
    );
    assert_eq!(
        d.saved_path(id, Path::new("/data")).unwrap(),
        Path::new("/data/courses/COMP30027/uploads/notes.pdf")
    );
}

#[test]
fn two_downloads_of_one_url_finish_in_order() {
    let mut d = Downloads::default();
    let (a, _) = d.request(1, 1, &url(URL), "a.pdf".into(), Path::new(ROOT));
    let (b, _) = d.request(2, 2, &url(URL), "a.pdf".into(), Path::new(ROOT));
    assert_eq!(finish(&mut d, true).unwrap().0, a);
    assert_eq!(finish(&mut d, false).unwrap().0, b);
    assert_eq!(state(&d, a), State::Ready);
    assert_eq!(state(&d, b), State::Failed);
}

#[test]
fn a_choice_needs_a_subject_or_a_full_path() {
    let (mut d, id, _) = started();
    assert!(d
        .choose(
            id,
            Dest::Oculus {
                subject_code: " ".into()
            }
        )
        .is_err());
    assert!(d
        .choose(
            id,
            Dest::Path {
                path: "notes.pdf".into()
            }
        )
        .is_err());
    assert!(d.choose(99, oculus()).is_err());
}

#[test]
fn only_a_settled_download_is_cleared() {
    let (mut d, id, _) = started();
    assert!(d.clear(id).is_err(), "receiving");
    finish(&mut d, true).unwrap();
    assert!(d.clear(id).is_err(), "ready");
    d.choose(id, Dest::Discard).unwrap();
    d.clear(id).unwrap();
    assert!(d.snapshot().downloads.is_empty());
}

#[test]
fn the_poller_runs_only_while_something_is_receiving() {
    let (mut d, id, staged) = started();
    assert!(d.start_polling());
    assert!(!d.start_polling(), "one poller");
    assert_eq!(d.poll().unwrap(), [(id, staged)]);
    assert!(d.set_received(&[(id, 10)]));
    assert!(!d.set_received(&[(id, 10)]), "unchanged is not re-sent");

    finish(&mut d, true).unwrap();
    assert!(d.poll().is_none(), "the poller ends");
    assert!(!d.set_received(&[(id, 99)]), "a finished size is final");

    d.request(7, 7, &url(URL), "b.pdf".into(), Path::new(ROOT));
    assert!(d.start_polling(), "a later download starts it again");
}

#[test]
fn a_download_from_a_closed_popup_belongs_to_its_opener() {
    let mut d = Downloads::default();
    let (id, _) = d.request(9, 4, &url(URL), "a.pdf".into(), Path::new(ROOT));
    assert_eq!(d.snapshot().downloads[0].tab, 4);
    assert_eq!(d.page_of(id), Some(9));
    assert!(d.unfinished_in(9));
    assert!(!d.unfinished_in(4));
}

#[test]
fn the_payload_carries_no_url_and_keeps_the_wire_names() {
    let (mut d, id, _) = started();
    d.choose(id, oculus()).unwrap();
    let json = serde_json::to_value(d.snapshot()).unwrap();
    let text = json.to_string();
    assert!(!text.contains("SECRET"), "{text}");
    assert!(!text.contains("/courses/1/files"), "{text}");
    let row = &json["downloads"][0];
    assert_eq!(row["host"], "files.example.com");
    assert_eq!(row["state"], "receiving");
    assert_eq!(row["can_retry"], true);
    assert_eq!(
        row["choice"],
        serde_json::json!({"kind": "oculus", "subject_code": "COMP30027"})
    );
    for (wire, dest) in [
        (
            serde_json::json!({"kind": "path", "path": "/tmp/a.pdf"}),
            Dest::Path {
                path: "/tmp/a.pdf".into(),
            },
        ),
        (serde_json::json!({"kind": "discard"}), Dest::Discard),
    ] {
        assert_eq!(serde_json::from_value::<Dest>(wire).unwrap(), dest);
    }
}

#[test]
fn the_startup_sweep_empties_the_staging_folder() {
    let data = crate::test_support::Scratch::new("download-sweep");
    let root = data.join("downloads").join(".incoming");
    std::fs::create_dir_all(root.join("123-4")).unwrap();
    std::fs::write(root.join("123-4").join("half.zip"), b"partial").unwrap();
    std::fs::write(data.join("downloads").join("keep"), b"not staging").unwrap();
    sweep_incoming(&root);
    assert!(!root.exists());
    assert!(data.join("downloads").join("keep").exists());
    sweep_incoming(&root);
}
