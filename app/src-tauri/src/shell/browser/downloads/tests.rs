use std::path::{Path, PathBuf};

use super::correlate::Ended;
use super::stage::source_of;
use super::state::{Effect, Settled, Source, Target};
use super::{host, origin, url_key, Dest, Downloads, SavedTo, State};
use crate::library::files::uploads::ImportOutcome;
use crate::shell::browser::Inner;

const ROOT: &str = "/data/downloads/.incoming";
const URL: &str = "https://files.example.com/courses/1/files/2/download?verifier=SECRET";
/// `settle` as WebKit reads a pathless failure (not ours) and as WebView2 does.
const MAC: bool = false;
const WINDOWS: bool = true;

fn url(s: &str) -> url::Url {
    s.parse().unwrap()
}

/// A download of `from` arriving in `page`, credited to `tab`.
fn source(page: u32, tab: u32, from: &str) -> Source {
    let from = url(from);
    Source {
        page,
        tab,
        url_key: url_key(&from),
        host: host(&from),
    }
}

fn request(d: &mut Downloads, page: u32, name: &str) -> (u32, PathBuf) {
    d.request(source(page, page, URL), name.into(), Path::new(ROOT))
}

/// One download of `URL` in tab 7.
fn started() -> (Downloads, u32, PathBuf) {
    let mut d = Downloads::default();
    let (id, staged) = request(&mut d, 7, "notes.pdf");
    (d, id, staged)
}

/// `Finished` for the download staged at `staged`, as patched WebKit and a
/// WebView2 success report it.
fn finish(d: &mut Downloads, staged: &Path, success: bool) -> Vec<Settled> {
    let ended = Ended {
        page: 7,
        url_key: url_key(&url(URL)),
        path: Some(staged),
        success,
    };
    d.finish_as(ended, MAC, |_| Some(42))
}

/// `Finished` with no path, for `from` in `page`.
fn pathless(d: &mut Downloads, page: u32, from: &str, success: bool, os: bool) -> Vec<Settled> {
    let ended = Ended {
        page,
        url_key: url_key(&url(from)),
        path: None,
        success,
    };
    d.finish_as(ended, os, |_| Some(42))
}

/// The effects of the one download a `Finished` settled.
fn effects(mut settled: Vec<Settled>) -> Vec<Effect> {
    assert_eq!(settled.len(), 1, "{settled:?}");
    settled.remove(0).effects
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
    let (second, other) = request(&mut d, 7, "notes.pdf");
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

    let settled = finish(&mut d, &staged, true);
    assert_eq!(
        settled,
        [Settled {
            id,
            page: 7,
            success: true,
            effects: vec![Effect::Deliver {
                id,
                staged: staged.clone(),
                to: Target::Oculus {
                    subject_code: "COMP30027".into()
                }
            }]
        }]
    );
    assert_eq!(state(&d, id), State::Delivering);
    assert_eq!(d.snapshot().downloads[0].received, 42, "the final size");

    assert!(d.choose(id, oculus()).is_err(), "no second delivery");
    assert!(
        finish(&mut d, &staged, true).is_empty(),
        "a second finish matches nothing"
    );
}

#[test]
fn a_finish_before_the_choice_waits_then_delivers_once() {
    // A small file's `Finished` arrives 1–2 ms after its `Requested`.
    let (mut d, id, staged) = started();
    assert!(effects(finish(&mut d, &staged, true)).is_empty());
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
    let (mut d, id, staged) = started();
    finish(&mut d, &staged, true);
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
        let (mut d, id, staged) = started();
        let mut all = vec![];
        if choose_first {
            all.extend(d.choose(id, oculus()).unwrap());
            all.extend(effects(finish(&mut d, &staged, true)));
        } else {
            all.extend(effects(finish(&mut d, &staged, true)));
            all.extend(d.choose(id, oculus()).unwrap());
        }
        let _ = d.choose(id, oculus());
        assert!(finish(&mut d, &staged, true).is_empty());
        assert_eq!(deliveries(&all), 1, "choose first: {choose_first}");
    }
}

#[test]
fn a_choice_can_change_while_bytes_arrive() {
    let (mut d, id, staged) = started();
    d.choose(id, oculus()).unwrap();
    d.choose(
        id,
        Dest::Path {
            path: "/tmp/notes.pdf".into(),
        },
    )
    .unwrap();
    assert!(matches!(
        &effects(finish(&mut d, &staged, true))[..],
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

    assert_eq!(
        effects(finish(&mut d, &staged, true)),
        [Effect::Remove(staged)]
    );
    assert_eq!(state(&d, id), State::Discarded);
    assert!(!d.unfinished_in(7));
}

#[test]
fn clearing_a_discarded_download_still_arriving_keeps_its_page_until_the_end() {
    let (mut d, id, staged) = started();
    d.choose(id, Dest::Discard).unwrap();
    d.clear(id).unwrap();
    assert!(d.snapshot().downloads.is_empty(), "gone from the list");
    assert!(d.unfinished_in(7), "but its end is still to come");
    assert!(d.choose(id, oculus()).is_err());

    let settled = finish(&mut d, &staged, true);
    assert_eq!(settled[0].page, 7, "the page to let go");
    assert_eq!(settled[0].effects, [Effect::Remove(staged.clone())]);
    assert!(!d.unfinished_in(7));
    assert!(d.clear(id).is_err(), "nothing is left of it");
    assert!(finish(&mut d, &staged, true).is_empty());
}

#[test]
fn discarding_a_ready_download_deletes_at_once() {
    let (mut d, id, staged) = started();
    finish(&mut d, &staged, true);
    assert_eq!(
        d.choose(id, Dest::Discard).unwrap(),
        [Effect::Remove(staged)]
    );
    assert_eq!(state(&d, id), State::Discarded);
}

#[test]
fn a_failed_download_keeps_no_bytes_and_takes_no_choice() {
    // `fail-drop`: the partial file stays on disk until it is removed.
    let (mut d, id, staged) = started();
    d.choose(id, oculus()).unwrap();
    assert_eq!(
        effects(finish(&mut d, &staged, false)),
        [Effect::Remove(staged)],
        "no delivery, the partial bytes go"
    );
    let row = &d.snapshot().downloads[0];
    assert_eq!(row.state, State::Failed);
    assert_eq!(row.choice, None);
    assert!(row
        .error
        .as_deref()
        .unwrap()
        .contains("again from the page"));
    assert!(d.choose(id, oculus()).is_err());
    assert!(!d.unfinished_in(7));
    d.clear(id).unwrap();
}

#[test]
fn a_delivery_that_does_not_land_is_ready_for_another_choice() {
    let (mut d, id, staged) = started();
    finish(&mut d, &staged, true);
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
    let (mut d, id, staged) = started();
    finish(&mut d, &staged, true);
    d.choose(id, oculus()).unwrap();
    assert!(
        d.saved_path(id, Path::new("/data")).is_err(),
        "not saved yet"
    );
    let outcome = ImportOutcome {
        source: "notes.pdf".into(),
        file: Some(crate::library::files::ImportedFile {
            filename: "notes.pdf".into(),
            relative_path: "courses/COMP30027/notes.pdf".into(),
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
        Path::new("/data/courses/COMP30027/notes.pdf")
    );
}

#[test]
fn one_url_downloading_twice_finishes_each_by_its_own_path() {
    // `same-url-twice`: the second finished while the first was 35% written.
    let mut d = Downloads::default();
    let (first, first_staged) = request(&mut d, 7, "same.zip");
    let (second, second_staged) = request(&mut d, 7, "same.zip");
    d.choose(first, oculus()).unwrap();

    let settled = finish(&mut d, &second_staged, true);
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].id, second);
    assert_eq!(state(&d, second), State::Ready);
    assert_eq!(
        state(&d, first),
        State::Receiving,
        "still being written: nothing may move it yet"
    );
    assert!(d.unfinished_in(7));

    let settled = finish(&mut d, &first_staged, true);
    assert_eq!(settled[0].id, first);
    assert_eq!(deliveries(&settled[0].effects), 1);
    assert!(!d.unfinished_in(7));
}

#[test]
fn a_finished_with_no_path_is_not_ours_on_webkit() {
    let (mut d, id, _) = started();
    // The orphan of a cross-origin `<a download>` redirect, and a download
    // the hook refused: `Finished { success: false }`, path `None`.
    assert!(pathless(&mut d, 7, "https://files.example.com/redirect", false, MAC).is_empty());
    assert!(pathless(&mut d, 7, URL, false, MAC).is_empty());
    assert_eq!(state(&d, id), State::Receiving);
    assert!(d.unfinished_in(7));
}

#[test]
fn without_the_wry_change_a_pathless_success_settles_only_a_whole_url_group() {
    let (mut d, id, _) = started();
    let (other, _) = request(&mut d, 7, "notes.pdf");
    assert!(
        pathless(&mut d, 7, URL, true, MAC).is_empty(),
        "one of two: which is unknown, and the other may be half-written"
    );
    let settled = pathless(&mut d, 7, URL, true, MAC);
    assert_eq!(settled.len(), 2, "both ended: every byte of both is in");
    assert_eq!(state(&d, id), State::Ready);
    assert_eq!(state(&d, other), State::Ready);

    let (lone, _) = request(&mut d, 7, "notes.pdf");
    assert_eq!(pathless(&mut d, 7, URL, true, MAC)[0].id, lone);
}

#[test]
fn on_webview2_a_pathless_failure_is_the_download_of_that_url_in_that_page() {
    let (mut d, id, staged) = started();
    let (elsewhere, _) = request(&mut d, 8, "notes.pdf");
    let settled = pathless(&mut d, 7, URL, false, WINDOWS);
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].effects, [Effect::Remove(staged)]);
    assert_eq!(state(&d, id), State::Failed);
    assert_eq!(state(&d, elsewhere), State::Receiving, "another page's");
}

#[test]
fn a_streaming_playlist_is_a_row_that_was_never_staged() {
    let mut d = Downloads::default();
    let hls = "https://media.example.com/hls/master.m3u8?token=SECRET";
    let id = d.refuse(source(7, 7, hls), "master.m3u8".into(), "a playlist");
    let row = d.snapshot().downloads[0].clone();
    assert_eq!(row.state, State::Refused);
    assert_eq!(row.error.as_deref(), Some("a playlist"));
    assert_eq!(row.host.as_deref(), Some("media.example.com"));
    assert_eq!(row.received, 0);
    assert!(!d.unfinished_in(7), "no end to wait for: its page can go");
    assert!(d.poll().is_none(), "nothing to measure");
    assert!(d.choose(id, oculus()).is_err());
    // The cancelled download's own `Finished`: path `None`, on either engine.
    assert!(pathless(&mut d, 7, hls, false, MAC).is_empty());
    assert!(pathless(&mut d, 7, hls, false, WINDOWS).is_empty());
    assert_eq!(state(&d, id), State::Refused);
    d.clear(id).unwrap();
    assert!(d.snapshot().downloads.is_empty());
}

#[test]
fn a_download_with_nowhere_to_arrive_fails_from_the_start() {
    let (mut d, id, _) = started();
    d.unstaged(id, "no room");
    let row = &d.snapshot().downloads[0];
    assert_eq!(row.state, State::Failed);
    assert_eq!(row.error.as_deref(), Some("no room"));
    assert!(!d.unfinished_in(7));
    assert!(d.poll().is_none());
    assert!(pathless(&mut d, 7, URL, false, WINDOWS).is_empty());
    d.clear(id).unwrap();
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
    let (mut d, id, staged) = started();
    assert!(d.clear(id).is_err(), "receiving");
    finish(&mut d, &staged, true);
    assert!(d.clear(id).is_err(), "ready");
    d.choose(id, Dest::Discard).unwrap();
    d.clear(id).unwrap();
    assert!(d.snapshot().downloads.is_empty());
    assert!(d.clear(id).is_err(), "gone");
}

#[test]
fn the_poller_runs_only_while_something_is_receiving() {
    let (mut d, id, staged) = started();
    assert!(d.start_polling());
    assert!(!d.start_polling(), "one poller");
    assert_eq!(d.poll().unwrap(), [(id, staged.clone())]);
    assert!(d.set_received(&[(id, 10)]));
    assert!(!d.set_received(&[(id, 10)]), "unchanged is not re-sent");

    finish(&mut d, &staged, true);
    assert!(d.poll().is_none(), "the poller ends");
    assert!(!d.set_received(&[(id, 99)]), "a finished size is final");

    request(&mut d, 7, "b.pdf");
    assert!(d.start_polling(), "a later download starts it again");
}

#[test]
fn a_tab_that_has_shown_no_page_is_download_only_with_or_without_an_opener() {
    let mut s = Inner::default();
    // Tab 9 was opened by tab 4's popup; tab 5 by a link from the app, a
    // typed address or a new tab; tab 6 has shown a page.
    s.blank.insert(9, Some(4));
    s.blank.insert(5, None);

    let (popup, download_only) = source_of(&s, 9, &url(URL));
    assert!(download_only);
    assert_eq!((popup.page, popup.tab), (9, 4), "credited to its opener");

    let (alone, download_only) = source_of(&s, 5, &url(URL));
    assert!(download_only);
    assert_eq!((alone.page, alone.tab), (5, 5), "its own, though it closes");

    let (page, download_only) = source_of(&s, 6, &url(URL));
    assert!(!download_only, "the tab stays on its page");
    assert_eq!((page.page, page.tab), (6, 6));

    // The page it arrives in is the one kept until it ends, not the opener's.
    let mut d = Downloads::default();
    d.request(popup, "a.pdf".into(), Path::new(ROOT));
    assert_eq!(d.snapshot().downloads[0].tab, 4);
    assert!(d.unfinished_in(9));
    assert!(!d.unfinished_in(4));
}

#[test]
fn a_closed_tab_is_no_longer_blank() {
    let mut s = Inner::default();
    s.blank.insert(5, None);
    s.forget(5);
    assert!(!source_of(&s, 5, &url(URL)).1);
}

#[test]
fn the_host_and_the_logged_origin_never_carry_the_rest_of_the_url() {
    let signed = url(URL);
    assert_eq!(host(&signed).as_deref(), Some("files.example.com"));
    assert_eq!(origin(&signed), "https://files.example.com");
    let blob = url("blob:https://app.example.com/0b1c3d4e-0000-4000-8000-000000000000");
    assert_eq!(host(&blob).as_deref(), Some("app.example.com"));
    assert_eq!(origin(&blob), "https://app.example.com");
    let data = url("data:application/zip;base64,UEsDBA==");
    assert_eq!(host(&data), None);
    assert_eq!(origin(&data), "data:");
}

#[test]
fn the_payload_carries_no_url_and_keeps_the_wire_names() {
    let (mut d, id, _) = started();
    d.choose(id, oculus()).unwrap();
    d.refuse(source(7, 7, URL), "master.m3u8".into(), "a playlist");
    let json = serde_json::to_value(d.snapshot()).unwrap();
    let text = json.to_string();
    assert!(!text.contains("SECRET"), "{text}");
    assert!(!text.contains("/courses/1/files"), "{text}");
    let row = &json["downloads"][0];
    let mut fields: Vec<&str> = row
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        ["choice", "error", "host", "id", "name", "received", "saved_to", "state", "tab"]
    );
    assert_eq!(row["host"], "files.example.com");
    assert_eq!(row["state"], "receiving");
    assert_eq!(
        row["choice"],
        serde_json::json!({"kind": "oculus", "subject_code": "COMP30027"})
    );
    assert_eq!(json["downloads"][1]["state"], "refused");
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
