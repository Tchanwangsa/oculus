//! Terminal output: colour, the sync reporter and the agent event printer.

use super::*;

pub(crate) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

/// Prints `harness::run_once` events. `show_text` is off where the reply is
/// JSON this binary prints itself (`oculus lecture`).
pub(crate) struct AgentPrinter {
    show_text: bool,
    /// The cursor is mid-line inside streamed text.
    mid: Mutex<bool>,
}

impl AgentPrinter {
    pub(crate) fn new(show_text: bool) -> Self {
        AgentPrinter {
            show_text,
            mid: Mutex::new(false),
        }
    }

    pub(crate) fn print(&self, ev: &app_lib::harness::HarnessEvent) {
        use app_lib::harness::HarnessEvent;
        let mut out = std::io::stdout();
        let mut mid = self.mid.lock().unwrap();
        let end_line = |mid: &mut bool, out: &mut std::io::Stdout| {
            if *mid {
                let _ = writeln!(out);
                *mid = false;
            }
        };
        match ev {
            HarnessEvent::SessionStarted {
                provider_session_id,
                model,
                cwd,
            } => {
                let _ = writeln!(
                    out,
                    "{} session {provider_session_id}{} in {cwd}",
                    paint("·", DIM),
                    model
                        .as_ref()
                        .map(|m| format!(" ({m})"))
                        .unwrap_or_default()
                );
            }
            HarnessEvent::AssistantDelta { text } => {
                if self.show_text {
                    let _ = write!(out, "{text}");
                    *mid = true;
                }
            }
            HarnessEvent::AssistantMessage { text } => {
                if self.show_text {
                    end_line(&mut mid, &mut out);
                } else {
                    let _ = writeln!(
                        out,
                        "{}",
                        paint(
                            &format!("  · replied ({} chars)", text.chars().count()),
                            DIM
                        )
                    );
                }
            }
            HarnessEvent::Thinking { text } => {
                end_line(&mut mid, &mut out);
                let first = text.lines().next().unwrap_or("");
                let _ = writeln!(out, "{}", paint(&format!("  thinking: {first}"), DIM));
            }
            HarnessEvent::ToolStarted { kind, title, .. } => {
                end_line(&mut mid, &mut out);
                let _ = writeln!(out, "{}", paint(&format!("  ▸ {kind:?}: {title}"), DIM));
            }
            HarnessEvent::ToolFinished { ok, output, .. } => {
                let first = output.lines().next().unwrap_or("");
                let mark = if *ok { "✓" } else { "✗" };
                let _ = writeln!(out, "{}", paint(&format!("    {mark} {first}"), DIM));
            }
            HarnessEvent::Usage {
                context_tokens,
                cost_usd,
                ..
            } => {
                end_line(&mut mid, &mut out);
                let mut s = String::from("  usage:");
                if let Some(c) = context_tokens {
                    s.push_str(&format!(" {c} context tokens"));
                }
                if let Some(c) = cost_usd {
                    s.push_str(&format!(", ${c:.3}"));
                }
                let _ = writeln!(out, "{}", paint(&s, DIM));
            }
            HarnessEvent::RateLimits { windows } => {
                let parts: Vec<String> = windows
                    .iter()
                    .map(|w| format!("{} {:.0}%", w.label, w.used_percent))
                    .collect();
                let _ = writeln!(
                    out,
                    "{}",
                    paint(&format!("  limits: {}", parts.join(", ")), DIM)
                );
            }
            HarnessEvent::Error { message, .. } => {
                end_line(&mut mid, &mut out);
                let _ = writeln!(out, "{} {message}", paint("error:", RED));
            }
            // Antigravity refused; the turn ends here.
            HarnessEvent::PermissionNeeded {
                action,
                target,
                rule,
                ..
            } => {
                end_line(&mut mid, &mut out);
                let what = target.as_deref().unwrap_or(action.as_str());
                let hint = rule
                    .as_deref()
                    .map(|r| format!(" — allow {r}"))
                    .unwrap_or_default();
                let _ = writeln!(out, "{} {what}{hint}", paint("refused:", RED));
            }
            HarnessEvent::TurnFinished { status } => {
                end_line(&mut mid, &mut out);
                let _ = writeln!(out, "{}", paint(&format!("· turn {status}"), DIM));
            }
            _ => {}
        }
        let _ = out.flush();
    }
}

pub(crate) use app_lib::lectures::chapters::hms as clock;

/// One line per artifact, remembered for one database write at the end.

pub(crate) struct TermReporter {
    phase: Mutex<String>,
    counter: Mutex<(usize, usize)>,
    written: std::sync::Arc<Mutex<Vec<FileEvent>>>,
    course: Mutex<String>,
}

impl TermReporter {
    pub(crate) fn new() -> Self {
        TermReporter {
            phase: Mutex::new(String::new()),
            counter: Mutex::new((0, 0)),
            written: Default::default(),
            course: Mutex::new(String::new()),
        }
    }
    pub(crate) fn sink(&self) -> std::sync::Arc<Mutex<Vec<FileEvent>>> {
        std::sync::Arc::clone(&self.written)
    }
}

impl Reporter for TermReporter {
    fn progress(&self, p: &Progress) {
        let mut course = self.course.lock().unwrap();
        if *course != p.course {
            *course = p.course.clone();
            println!("{}", paint(&p.course, BOLD));
        }
        *self.phase.lock().unwrap() = p.phase.clone();
        *self.counter.lock().unwrap() = (p.done, p.total);
    }

    fn file(&self, f: &FileEvent) {
        let (done, total) = *self.counter.lock().unwrap();
        let phase = self.phase.lock().unwrap().clone();
        let counter = if phase == "modules" && total > 0 {
            format!("{done}/{total}")
        } else {
            String::new()
        };
        // The course is already the section header, so drop `courses/CODE/`.
        let short = f
            .relative_path
            .splitn(3, '/')
            .nth(2)
            .unwrap_or(&f.relative_path);
        // Pad before painting: escape codes count toward a width specifier.
        println!(
            "  {} {}  {short:<52} {}",
            paint(&format!("{phase:<13}"), DIM),
            paint(&format!("{counter:>7}"), DIM),
            paint(&format!("{:>9}", human_bytes(f.size_bytes)), DIM)
        );
        let _ = std::io::stdout().flush();
        self.written.lock().unwrap().push(f.clone());
    }

    fn file_failed(&self, f: &sync::FileFailed) {
        let short = f
            .relative_path
            .splitn(3, '/')
            .nth(2)
            .unwrap_or(&f.relative_path);
        eprintln!("  {} {short}: {}", paint("error", RED), f.error);
    }

    fn log(&self, level: &str, course: &str, message: &str) {
        let tag = match level {
            "error" => paint("error", RED),
            "warning" => paint("warn", YELLOW),
            _ => paint("info", DIM),
        };
        eprintln!("  {tag} {course}: {message}");
    }
}

pub(crate) fn human_bytes(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

pub(crate) const DIM: &str = "\x1b[2m";
pub(crate) const BOLD: &str = "\x1b[1m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const YELLOW: &str = "\x1b[33m";

pub(crate) fn colour_ok() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal())
}

pub(crate) fn paint(s: &str, code: &str) -> String {
    if s.is_empty() || !colour_ok() {
        s.to_string()
    } else {
        format!("{code}{s}\x1b[0m")
    }
}
