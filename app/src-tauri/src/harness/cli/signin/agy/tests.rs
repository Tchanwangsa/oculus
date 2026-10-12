//! `Screen` over real `agy` 1.3.3 output, captured on a PTY with a signed-out
//! HOME (`code_challenge` and `state` redacted, one placeholder per round).

use super::screen::{render, Phase, Screen, Step};

/// 400 columns: menu, URL, a bad code, the error, Enter, the menu again, a
/// second bad code, the error again.
const TWO_REFUSED: &[u8] = include_bytes!("fixtures/two-refused-codes.out");
/// 120 columns, so the visible URL wraps over six lines: one bad code.
const NARROW: &[u8] = include_bytes!("fixtures/narrow-refused-code.out");

const REFUSAL: &str =
    r#"Got an error: token exchange failed: oauth2: "invalid_grant" "Malformed auth code.""#;

/// Feed `raw` a byte at a time, pasting the next code whenever the code box
/// is up — as the student would, before agy says anything more.
fn drive(raw: &[u8], codes: &[&str]) -> Vec<Step> {
    let mut screen = Screen::new();
    let mut codes = codes.iter();
    let mut steps = Vec::new();
    for b in raw {
        steps.extend(screen.feed(std::slice::from_ref(b)));
        if screen.phase() == Phase::AwaitingCode {
            if let Some(code) = codes.next() {
                screen.code_sent(code).unwrap();
            }
        }
    }
    steps
}

fn urls(steps: &[Step]) -> Vec<&str> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Url(u) => Some(u.as_str()),
            _ => None,
        })
        .collect()
}

fn lines(steps: &[Step]) -> Vec<&str> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Line(l) => Some(l.as_str()),
            _ => None,
        })
        .collect()
}

fn count(steps: &[Step], pred: impl Fn(&Step) -> bool) -> usize {
    steps.iter().filter(|s| pred(s)).count()
}

#[test]
fn one_run_takes_a_second_code_after_a_refusal() {
    let steps = drive(TWO_REFUSED, &["bogus-not-a-real-code", "bogus-two"]);

    // Menu, "any key", menu again, "any key" again.
    assert_eq!(count(&steps, |s| *s == Step::Key(b"\r")), 4, "{steps:#?}");
    assert_eq!(
        steps
            .iter()
            .filter(|s| matches!(s, Step::Refused(_)))
            .collect::<Vec<_>>(),
        vec![
            &Step::Refused(REFUSAL.to_string()),
            &Step::Refused(REFUSAL.to_string())
        ]
    );
    assert_eq!(count(&steps, |s| matches!(s, Step::Fatal(_))), 0);

    // Each round has its own URL, read whole from the hyperlink.
    let urls = urls(&steps);
    assert_eq!(urls.len(), 2, "{urls:?}");
    for (i, url) in urls.iter().enumerate() {
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/auth?access_type=offline"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fantigravity.google%2Foauth-callback"));
        assert!(url.contains(&format!("code_challenge=REDACTED{}", i + 1)));
        assert!(url.ends_with(&format!("state=REDACTED{}", i + 1)), "{url}");
    }

    let lines = lines(&steps);
    for want in [
        "Welcome to the Antigravity CLI. You are currently not signed in.",
        "Select login method:",
        "Signing in...",
        "Your browser should open automatically. If not:",
        "If you aren't automatically redirected, paste the authorization code below:",
    ] {
        assert!(lines.contains(&want), "missing {want:?} in {lines:#?}");
    }
    // The refusal shows once per round; nothing else repeats within one.
    assert_eq!(lines.iter().filter(|l| **l == REFUSAL).count(), 2);
    for line in &lines {
        let lower = line.to_lowercase();
        for noise in ["bogus", "navigate", "https://", "press any key", "──"] {
            assert!(!lower.contains(noise), "{line:?} carries {noise:?}");
        }
    }
}

#[test]
fn a_wrapped_url_is_still_read_whole() {
    let steps = drive(NARROW, &["bogus-not-a-real-code"]);
    let urls = urls(&steps);
    assert_eq!(urls.len(), 1);
    assert!(urls[0].contains("&scope=https%3A%2F%2Fwww.googleapis.com%2Fauth%2Fcloud-platform"));
    assert!(urls[0].ends_with("state=REDACTED1"));
    assert_eq!(count(&steps, |s| *s == Step::Refused(REFUSAL.into())), 1);
    // The echo of the typed code is not output.
    assert!(lines(&steps).iter().all(|l| !l.contains("bog")));
}

/// Fed in one piece, no code was ever pasted: the error is not a refused
/// code, and the run must end rather than loop through the menu.
#[test]
fn an_error_without_a_pasted_code_ends_the_run() {
    let mut screen = Screen::new();
    let steps = screen.feed(TWO_REFUSED);
    assert_eq!(
        steps.iter().find(|s| matches!(s, Step::Fatal(_))),
        Some(&Step::Fatal(REFUSAL.to_string()))
    );
    assert_eq!(count(&steps, |s| matches!(s, Step::Refused(_))), 0);
    // Only the first menu is answered; "any key" waits on a refusal.
    assert_eq!(count(&steps, |s| *s == Step::Key(b"\r")), 1);
}

/// Keys may come sooner when a prompt arrives in pieces (it is read off the
/// incomplete tail), but no step is gained, lost or reordered otherwise.
#[test]
fn chunk_boundaries_do_not_change_the_reading() {
    let split = |steps: Vec<Step>| -> (Vec<Step>, usize) {
        let keys = count(&steps, |s| matches!(s, Step::Key(_)));
        let rest = steps
            .into_iter()
            .filter(|s| !matches!(s, Step::Key(_)))
            .collect();
        (rest, keys)
    };
    let whole = split(Screen::new().feed(TWO_REFUSED));
    for size in [1, 3, 64, 4096] {
        let mut screen = Screen::new();
        let mut steps = Vec::new();
        for chunk in TWO_REFUSED.chunks(size) {
            steps.extend(screen.feed(chunk));
        }
        assert_eq!(split(steps), whole, "chunk {size}");
    }
}

#[test]
fn a_redrawn_menu_is_answered_once() {
    let mut screen = Screen::new();
    let mut steps = screen.feed(b" Select login method:\r\n > 1. Google OAuth\n");
    steps.extend(screen.feed(b"\x1b[H Select login method:\r\n"));
    assert_eq!(count(&steps, |s| *s == Step::Key(b"\r")), 1);
    assert_eq!(screen.phase(), Phase::Opening);
}

#[test]
fn a_code_waits_for_the_code_box() {
    let mut screen = Screen::new();
    assert!(screen.code_sent("abc").is_err());
    screen.feed(b" Select login method:\r\n");
    assert!(screen.code_sent("abc").is_err());
    screen.feed(b" paste the authorization code below:\r\n");
    assert_eq!(screen.phase(), Phase::AwaitingCode);
    assert!(screen.code_sent("abc").is_ok());
    assert_eq!(screen.phase(), Phase::Exchanging);
}

/// Not seen live (that needs a real sign-in); the wording is agy's own,
/// from its binary.
#[test]
fn a_success_line_hints_at_the_outcome() {
    let mut screen = Screen::new();
    screen.feed(b" Select login method:\r\n paste the authorization code below:\r\n");
    screen.code_sent("4/0Abc").unwrap();
    let steps = screen.feed(b"OAuth: authenticated successfully as someone@example.com\r\n");
    assert!(steps.contains(&Step::SignedInHint), "{steps:?}");
}

#[test]
fn a_plain_url_is_the_fallback_without_a_hyperlink() {
    let mut screen = Screen::new();
    screen.feed(b" Select login method:\r\n");
    let steps = screen.feed(b"  \x1b[4mhttps://accounts.google.com/o/oauth2/auth?x=1\x1b[m\r\n");
    assert_eq!(
        urls(&steps),
        vec!["https://accounts.google.com/o/oauth2/auth?x=1"]
    );
}

#[test]
fn render_cuts_at_cursor_moves_and_drops_link_text() {
    let r = render(
        "a\x1b[2;1Hb\x1b[4Gc\x1bMd\x1b]8;id=1;https://x.example/y\x1b\\link text\x1b]8;;\x1b\\e\r\n",
    );
    assert_eq!(r.lines, vec!["a", "b c", "de"]);
    assert_eq!(r.links, vec!["https://x.example/y"]);
}
