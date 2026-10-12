//! Reading `agy`'s full-screen sign-in from its raw terminal output.
//!
//! The TUI redraws by cursor moves, not lines, so output is cut into lines
//! at every newline, carriage return and cursor-moving escape, and each
//! distinct line is passed on once per round. The authorize URL is read
//! from the OSC 8 hyperlink around it; the visible copy is wrapped.

use std::collections::HashSet;

use super::super::output::extract_url;

/// Where the sign-in stands, as far as the screen shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Phase {
    /// Waiting for the login-method menu.
    Menu,
    /// Menu answered; agy is opening the browser.
    Opening,
    /// The code box is showing.
    AwaitingCode,
    /// A code was sent; agy is exchanging it.
    Exchanging,
    /// agy refused the code and waits on "Press any key to go back".
    Refused,
    /// The key was sent; the menu comes back next.
    Returning,
}

/// What the supervisor does with a chunk of output.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Step {
    /// A readable line for the dialog's output.
    Line(String),
    /// The authorize URL, each time it changes (every round has its own).
    Url(String),
    /// Bytes to type into the terminal.
    Key(&'static [u8]),
    /// agy refused a pasted code with this sentence; it is going round again.
    Refused(String),
    /// agy failed outside a code exchange; the run ends with this sentence.
    Fatal(String),
    /// A line that says the exchange worked; `agy models` confirms it.
    SignedInHint,
}

/// Enter: picks the menu's first entry (Google OAuth), dismisses the error.
const ENTER: &[u8] = b"\r";

/// Lines that are key hints or input chrome, never news.
const CHROME: &[&str] = &[
    "navigate",
    "authorization code...",
    "press any key",
    "copy and paste the url",
];

const SUCCESS: &[&str] = &[
    "authenticated successfully",
    "successfully exchanged authorization code",
];

pub(super) struct Screen {
    /// Bytes after the last newline, held until the line completes.
    pending: Vec<u8>,
    phase: Phase,
    url: Option<String>,
    /// Lines already passed on this round.
    shown: HashSet<String>,
    /// The last code sent, so its echo in the input box is not passed on.
    code: Option<String>,
}

impl Screen {
    pub(super) fn new() -> Self {
        Screen {
            pending: Vec::new(),
            phase: Phase::Menu,
            url: None,
            shown: HashSet::new(),
            code: None,
        }
    }

    pub(super) fn phase(&self) -> Phase {
        self.phase
    }

    /// Note a code about to be typed. Refused unless the code box is up: in
    /// the menu or on the error screen the keystrokes would mean something
    /// else.
    pub(super) fn code_sent(&mut self, code: &str) -> Result<(), String> {
        if self.phase != Phase::AwaitingCode {
            return Err("Antigravity is not waiting for a code yet".into());
        }
        self.code = Some(squash(code));
        self.phase = Phase::Exchanging;
        Ok(())
    }

    /// The code [`Self::code_sent`] noted never reached agy; the box is
    /// still waiting.
    pub(super) fn code_lost(&mut self) {
        if self.phase == Phase::Exchanging {
            self.phase = Phase::AwaitingCode;
        }
    }

    /// Take a chunk of raw output; returns what to do about it, in order.
    pub(super) fn feed(&mut self, bytes: &[u8]) -> Vec<Step> {
        self.pending.extend_from_slice(bytes);
        let mut steps = Vec::new();
        while let Some(i) = self.pending.iter().position(|&b| b == b'\n') {
            let segment: Vec<u8> = self.pending.drain(..=i).collect();
            let rendered = render(&String::from_utf8_lossy(&segment));
            self.take_links(&rendered.links, &mut steps);
            for line in rendered.lines {
                self.take_line(&line, &mut steps);
            }
        }
        // agy's last words before it waits ("Press any key to go back.")
        // carry no newline; read the incomplete tail for prompts only.
        let tail = render(&String::from_utf8_lossy(&self.pending));
        for line in tail.lines {
            self.take_prompt(&line.to_lowercase(), &mut steps);
        }
        steps
    }

    fn take_links(&mut self, links: &[String], steps: &mut Vec<Step>) {
        if let Some(url) = links.iter().find(|u| u.starts_with("https://")) {
            self.take_url(url, steps);
        }
    }

    fn take_url(&mut self, url: &str, steps: &mut Vec<Step>) {
        if self.url.as_deref() != Some(url) {
            self.url = Some(url.to_string());
            steps.push(Step::Url(url.to_string()));
        }
    }

    fn take_line(&mut self, line: &str, steps: &mut Vec<Step>) {
        let line = tidy(line);
        if line.is_empty() {
            return;
        }
        let lower = line.to_lowercase();
        self.take_prompt(&lower, steps);

        if lower.contains("paste the authorization code") && self.phase == Phase::Opening {
            self.phase = Phase::AwaitingCode;
        }
        if lower.starts_with("got an error") {
            if self.phase == Phase::Exchanging {
                self.phase = Phase::Refused;
                steps.push(Step::Refused(line.clone()));
            } else {
                steps.push(Step::Fatal(line.clone()));
            }
        }
        if SUCCESS.iter().any(|s| lower.contains(s)) {
            steps.push(Step::SignedInHint);
        }
        // Without a hyperlink, the visible URL is the fallback; it is only
        // whole when the terminal is wide enough not to wrap it.
        if self.url.is_none() && lower.contains("https://") {
            if let Some(url) = extract_url(&line) {
                self.take_url(&url, steps);
            }
        }

        if !worth_showing(&line, &lower, self.code.as_deref()) {
            return;
        }
        if self.shown.insert(line.clone()) {
            steps.push(Step::Line(line));
        }
    }

    /// The two screens answered by a key. Each fires once per round: the
    /// TUI redraws them, and a second Enter would land somewhere else.
    fn take_prompt(&mut self, lower: &str, steps: &mut Vec<Step>) {
        if lower.contains("select login method")
            && matches!(self.phase, Phase::Menu | Phase::Returning)
        {
            self.phase = Phase::Opening;
            steps.push(Step::Key(ENTER));
        } else if lower.contains("press any key") && self.phase == Phase::Refused {
            self.phase = Phase::Returning;
            // A new round: its lines (and a second refusal) show again.
            self.shown.clear();
            steps.push(Step::Key(ENTER));
        }
    }
}

/// Whether a tidied line is news for the dialog: it has words, is not key
/// hints or input chrome, and is not the pasted code's echo.
fn worth_showing(line: &str, lower: &str, code: Option<&str>) -> bool {
    if !line.chars().any(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    if CHROME.iter().any(|c| lower.contains(c)) {
        return false;
    }
    match code {
        Some(code) => !code.contains(&squash(line)),
        None => true,
    }
}

/// Trim, and drop the braille spinner frame some lines start with.
fn tidy(line: &str) -> String {
    line.trim()
        .trim_start_matches(|c: char| ('\u{2800}'..='\u{28ff}').contains(&c))
        .trim()
        .to_string()
}

/// Without whitespace: the input box echoes a code with cursor gaps in it.
fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// A stretch of terminal output as readable lines plus its hyperlink targets.
#[derive(Default, Debug)]
pub(super) struct Rendered {
    pub(super) lines: Vec<String>,
    pub(super) links: Vec<String>,
}

/// Cut terminal output into lines: newlines, carriage returns and every
/// cursor-moving escape end one. Colour and mode escapes vanish; a
/// hyperlink's visible text is dropped (it is the URL again, wrapped, or
/// "Click here") and its target kept.
pub(super) fn render(raw: &str) -> Rendered {
    let mut out = Rendered::default();
    let mut line = String::new();
    let mut in_link = false;
    let mut chars = raw.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    let mut last = None;
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            last = Some(c);
                            break;
                        }
                    }
                    match last {
                        // Cursor position, row, up/down, erase display,
                        // insert/delete lines: the next text is elsewhere.
                        Some('H' | 'f' | 'd' | 'A' | 'B' | 'E' | 'F' | 'J' | 'L' | 'M') => {
                            end_line(&mut line, &mut out)
                        }
                        // Column moves indent; a space keeps words apart.
                        Some('G' | 'C') => line.push(' '),
                        _ => {}
                    }
                }
                Some(']') => {
                    chars.next();
                    let mut body = String::new();
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' {
                            break;
                        }
                        if c == '\u{1b}' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        body.push(c);
                    }
                    // OSC 8: `8;params;uri` opens a link, `8;;` closes it.
                    if let Some(rest) = body.strip_prefix("8;") {
                        let uri = rest.split_once(';').map_or("", |(_, u)| u);
                        in_link = !uri.is_empty();
                        if in_link {
                            out.links.push(uri.to_string());
                        }
                    }
                }
                // Reverse index: a line above.
                Some('M') => {
                    chars.next();
                    end_line(&mut line, &mut out);
                }
                // Charset designations take one more character.
                Some('(' | ')' | '*' | '+') => {
                    chars.next();
                    chars.next();
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            },
            '\r' | '\n' => end_line(&mut line, &mut out),
            '\u{8}' => {
                if !in_link {
                    line.pop();
                }
            }
            c if c.is_control() => {}
            c => {
                if !in_link {
                    line.push(c);
                }
            }
        }
    }
    end_line(&mut line, &mut out);
    out
}

fn end_line(line: &mut String, out: &mut Rendered) {
    if line.trim().is_empty() {
        line.clear();
    } else {
        out.lines.push(std::mem::take(line));
    }
}
