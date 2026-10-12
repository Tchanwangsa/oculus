//! One render per folder: a second start is refused naming the first, and
//! the claim is freed however the render ends.

use std::sync::atomic::Ordering;

use super::super::{Preset, Renders};
use crate::test_support::Scratch;

/// A render that claims `dir`, then fails or finishes.
fn job(renders: &Renders, dir: &std::path::Path, fail: bool) -> Result<(), String> {
    let _running = renders.start(dir, Preset::Final)?;
    if fail {
        return Err("ffmpeg exited 1".into());
    }
    Ok(())
}

#[test]
fn a_second_start_for_the_same_folder_is_refused_naming_the_first() {
    let dir = Scratch::new("studio-running");
    let renders = Renders::new();
    let first = renders.start(&dir, Preset::Draft).unwrap();
    let again = renders
        .start(&dir.join("."), Preset::Final)
        .err()
        .expect("refused");
    let canonical = std::fs::canonicalize(&*dir).unwrap();
    assert!(again.contains("a draft render of"), "{again}");
    assert!(again.contains(&canonical.display().to_string()), "{again}");

    let other = Scratch::new("studio-running-other");
    drop(renders.start(&other, Preset::Final).unwrap());
    drop(first);
    drop(renders.start(&dir, Preset::Final).unwrap());
}

#[test]
fn cancel_sets_the_running_flag_and_the_guard_frees_the_folder() {
    let dir = Scratch::new("studio-running");
    let renders = Renders::new();
    assert!(!renders.cancel(&dir));
    let running = renders.start(&dir, Preset::Draft).unwrap();
    assert!(!running.cancel_flag().load(Ordering::Relaxed));
    assert!(renders.cancel(&dir));
    assert!(running.cancel_flag().load(Ordering::Relaxed));
    drop(running);
    assert!(!renders.cancel(&dir));
    let next = renders.start(&dir, Preset::Draft).unwrap();
    assert!(!next.cancel_flag().load(Ordering::Relaxed));
}

#[test]
fn a_failed_or_panicking_render_frees_the_folder() {
    let dir = Scratch::new("studio-running");
    let renders = Renders::new();
    assert_eq!(job(&renders, &dir, true).unwrap_err(), "ffmpeg exited 1");
    job(&renders, &dir, false).unwrap();

    let panicked = std::panic::catch_unwind(|| {
        let _running = renders.start(&dir, Preset::Draft).unwrap();
        panic!("worker panic");
    });
    assert!(panicked.is_err());
    job(&renders, &dir, false).unwrap();
}

#[test]
fn a_missing_folder_is_refused() {
    let dir = Scratch::new("studio-running");
    let renders = Renders::new();
    let e = renders
        .start(&dir.join("absent"), Preset::Draft)
        .err()
        .expect("refused");
    assert!(e.contains("absent"), "{e}");
}
