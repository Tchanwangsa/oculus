//! `oculus` — the Oculus command line.
//!
//! Same engine the app runs, without the window: it reads the session cookie
//! and the database the app already maintains, so a CLI sync and an in-app sync
//! are the same operation and either can follow the other.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use app_lib::store;
use app_lib::sync::{self, Engine, FileEvent, Progress, Reporter};
use clap::{Args, Parser, Subcommand};
use sqlx::SqlitePool;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(
    name = "oculus",
    version,
    about = "Sync Canvas subjects and lectures into your local Oculus library"
)]
struct Cli {
    /// Whole sidecar process-tree memory cap in MB (minimum 5120)
    #[arg(long, global = true, value_parser = clap::value_parser!(u64).range(5120..))]
    memory_cap: Option<u64>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn memory_cap_is_global_and_has_a_floor() {
        for arguments in [
            vec!["oculus", "--memory-cap", "8192", "index"],
            vec!["oculus", "index", "--memory-cap", "5120"],
        ] {
            let cli = Cli::try_parse_from(arguments).expect("global memory cap");
            assert!(cli.memory_cap.unwrap() >= 5120);
            assert!(matches!(cli.command, Some(Command::Index(_))));
        }
        assert!(Cli::try_parse_from(["oculus", "--memory-cap", "4096"]).is_err());
    }
}

#[derive(Subcommand)]
enum Command {
    /// Session, library and sidecar status
    Status,
    /// Sign in to Canvas, or sign out
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// List subjects or lectures
    List(ListArgs),
    /// Scrape Canvas content or sync lectures
    Run(RunArgs),
    /// Re-parse and re-embed PDFs already on record
    Index(IndexArgs),
}

#[derive(Args)]
struct IndexArgs {
    /// Subject codes to index. Omit for every subject.
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
}

#[derive(Subcommand)]
enum AuthAction {
    /// Open the app's Canvas sign-in window and wait for the session
    Login,
    /// Forget the saved session
    Logout,
    /// Whether the saved session still works
    Status,
    /// Store the credentials that let Oculus sign in without a browser
    ///
    /// Needs a TOTP factor (Google Authenticator) enrolled and its setup key.
    /// A code cannot be derived from other codes, so the key must come from
    /// the enrolment screen — re-enrol the factor if you never copied it.
    Setup,
    /// Sign in headlessly with the stored credentials, now
    Auto,
    /// One keep-alive cycle: roll the session forward, rebuild it if it died
    ///
    /// What the LaunchAgent runs every few hours. Prints nothing and always
    /// exits 0 — it reports into `session-keepalive.log` in the data dir,
    /// because launchd has nowhere to show a failure and a non-zero exit only
    /// makes launchd think the job crashed.
    Tick,
    /// Forget the stored sign-in credentials
    Forget,
    /// Report what the Okta sign-in page looks like, when `auto` fails
    Diagnose,
    /// Show the Ed Discussion session status, or set a token manually
    ///
    /// Normally unnecessary — syncs mint the Ed session from the Canvas
    /// session via the course's LTI launch. The manual token (DevTools →
    /// Network → any edstem /api request → `x-token` header) is an override.
    Ed {
        /// An x-token JWT to save. Omit to check the current session.
        token: Option<String>,
    },
}

#[derive(Args)]
struct ListArgs {
    /// List subjects (default)
    #[arg(short = 's', long)]
    subjects: bool,
    /// List lectures, optionally filtered to the given subject codes
    #[arg(short = 'l', long)]
    lectures: bool,
    /// Refresh the subject list from Canvas before printing
    #[arg(long)]
    refresh: bool,
    /// Subject codes to filter by
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
}

#[derive(Args)]
struct RunArgs {
    /// Scrape Canvas content: pages, announcements, modules, PDFs (default)
    #[arg(short = 's', long)]
    subjects: bool,
    /// Sync the Echo360 lecture list for the given subjects
    #[arg(short = 'l', long)]
    lectures: bool,
    /// Include subjects from past terms, not just the current one
    #[arg(long)]
    all: bool,
    /// Skip the sidecar entirely: no PDF parsing and no embedding
    #[arg(long)]
    no_parse: bool,
    /// Parse PDFs but do not embed them into the retrieval index
    #[arg(long)]
    no_embed: bool,
    /// With -l: also download and trim the lecture videos
    #[arg(long)]
    videos: bool,
    /// With -l: also download the lecture transcripts
    #[arg(long)]
    transcripts: bool,
    /// Subject codes to sync. Omit for every selected current subject.
    #[arg(value_name = "SUBJECT_CODE")]
    codes: Vec<String>,
}

fn main() {
    restore_sigpipe();
    let cli = Cli::parse();
    let ctx = Ctx::new();

    if let Some(cap) = cli.memory_cap {
        if let Err(error) = app_lib::sidecar::set_limits(Some(cap), None) {
            eprintln!("{} {error}", paint("error:", RED));
            std::process::exit(1);
        }
    }

    let result = match cli.command {
        None | Some(Command::Status) => ctx.status(),
        Some(Command::Auth { action }) => match action {
            AuthAction::Login => ctx.login(),
            AuthAction::Logout => ctx.logout(),
            AuthAction::Status => ctx.auth_status(),
            AuthAction::Setup => ctx.auth_setup(),
            AuthAction::Auto => ctx.auth_auto(),
            AuthAction::Tick => ctx.auth_tick(),
            AuthAction::Forget => ctx.auth_forget(),
            AuthAction::Diagnose => {
                print!("{}", app_lib::okta::diagnose());
                Ok(())
            }
            AuthAction::Ed { token } => ctx.auth_ed(token.as_deref()),
        },
        Some(Command::List(args)) => ctx.list(args),
        Some(Command::Run(args)) => ctx.run(args),
        Some(Command::Index(args)) => ctx.index(&args),
    };

    if let Err(e) = result {
        eprintln!("{} {e}", paint("error:", RED));
        std::process::exit(1);
    }
}

fn read_line(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim().to_string())
}

/// Prompt without echoing. `stty` rather than a crate — this is the only
/// place in the CLI that needs it, and echo is restored even if the read
/// fails, so a stray Ctrl-C cannot leave the terminal mute.
fn read_secret(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok();

    let hidden = std::process::Command::new("stty").arg("-echo").status().is_ok();
    let mut line = String::new();
    let read = std::io::stdin().read_line(&mut line);
    if hidden {
        std::process::Command::new("stty").arg("echo").status().ok();
        println!();
    }
    read.map_err(|e| e.to_string())?;

    let value = line.trim().to_string();
    if value.is_empty() {
        return Err("nothing entered".to_string());
    }
    Ok(value)
}

/// Rust ignores SIGPIPE at startup, which turns `oculus list | head` into a
/// panic on a closed pipe instead of a quiet exit. Put the default back.
fn restore_sigpipe() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

// ── Shared context ───────────────────────────────────────────────────────────

struct Ctx {
    data_dir: PathBuf,
    rt: Runtime,
}

impl Ctx {
    fn new() -> Self {
        Ctx {
            data_dir: app_lib::paths::data_dir(),
            rt: Runtime::new().expect("tokio runtime"),
        }
    }

    fn engine(&self, parse: bool) -> Engine {
        Engine::new(&self.data_dir, Box::new(TermReporter::new())).with_pdf_parsing(parse)
    }

    /// The database, or `None` with a warning printed. A missing database is
    /// not fatal: scraping still writes the library to disk.
    fn db(&self) -> Option<SqlitePool> {
        match self.rt.block_on(store::open(&self.data_dir)) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("{} {e}", paint("warning:", YELLOW));
                None
            }
        }
    }

    // ── status ───────────────────────────────────────────────────────────────

    fn status(&self) -> Result<(), String> {
        println!("{}  {}", paint("library", DIM), self.data_dir.display());

        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        match canvas.whoami() {
            Ok(name) => println!("{}   {} as {name}", paint("canvas", DIM), paint("connected", GREEN)),
            Err(e) if !canvas.has_session() => {
                println!("{}   {} — run `oculus auth login`", paint("canvas", DIM), paint("signed out", YELLOW));
                let _ = e;
            }
            Err(e) => println!("{}   {} — {e}", paint("canvas", DIM), paint("unusable", RED)),
        }

        let ed = app_lib::ed::Ed::open(&self.data_dir);
        if ed.has_session() {
            match ed.whoami() {
                Ok(name) => println!("{}       {} as {name}", paint("ed", DIM), paint("connected", GREEN)),
                Err(e) => println!("{}       {} — {e}", paint("ed", DIM), paint("unusable", RED)),
            }
        } else {
            println!(
                "{}       {} — connects automatically on the next sync",
                paint("ed", DIM),
                paint("not connected", DIM)
            );
        }

        // Report the pid: a sidecar that outlived its app answers /health
        // perfectly while serving stale code, and this is the only way to see
        // that from outside.
        let health: Option<serde_json::Value> = ureq::get(&format!(
            "http://127.0.0.1:{}/health",
            app_lib::sidecar::SIDECAR_PORT
        ))
        .timeout(std::time::Duration::from_millis(500))
        .call()
        .ok()
        // ureq's json helpers are behind a feature this crate does not enable.
        .and_then(|r| r.into_string().ok())
        .and_then(|s| serde_json::from_str(&s).ok());

        println!(
            "{}  {}",
            paint("sidecar", DIM),
            match &health {
                Some(h) => paint(
                    &format!(
                        "running (pid {}, parser v{})",
                        h["pid"].as_i64().unwrap_or(0),
                        h["parser_version"].as_i64().unwrap_or(1)
                    ),
                    GREEN
                ),
                None => paint("not running — PDFs will not be parsed", YELLOW),
            }
        );

        if let Some(pool) = self.db() {
            self.rt.block_on(async {
                let subjects = store::subjects(&pool).await.unwrap_or_default();
                let current = subjects.iter().filter(|s| s.is_current).count();
                let synced = subjects.iter().filter(|s| s.last_synced_at.is_some()).count();
                println!(
                    "{} {} ({current} current, {synced} synced)",
                    paint("subjects", DIM),
                    subjects.len()
                );
                let files: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);
                let parsed: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM files WHERE parse_status IN ('fast','quality')",
                )
                .fetch_one(&pool)
                .await
                .unwrap_or(0);
                println!("{}    {files} ({parsed} parsed)", paint("files", DIM));
            });
        }
        Ok(())
    }

    fn auth_status(&self) -> Result<(), String> {
        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        match canvas.whoami() {
            Ok(name) => {
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Walk the user through storing credentials. The instructions matter as
    /// much as the prompts: the setup key is the one thing that cannot be
    /// recovered later, and the second QR scan is what stops this from being
    /// a one-way door out of your own account.
    fn auth_setup(&self) -> Result<(), String> {
        println!("{}", paint("Automated sign-in setup", DIM));
        println!();
        println!("This needs a TOTP factor enrolled and its setup key. A TOTP code is a");
        println!("one-way function of a secret seed, so it cannot be worked out from other");
        println!("codes — the seed is shown only at enrolment. If you never copied it:");
        println!();
        println!("  1. Open https://sso.unimelb.edu.au/enduser/settings");
        println!("  2. Google Authenticator → remove it, then set it up again");
        println!("  3. On the QR page click \"Can't scan?\" to reveal the setup key");
        println!("  4. {} scan the QR with your phone as well, so you keep", paint("Also", YELLOW));
        println!("     a working authenticator if this Mac is ever unavailable.");
        println!();

        let username = read_line("Username (e.g. chanwangsat): ")?;
        let password = read_secret("Password: ")?;
        let secret = read_secret("Authenticator setup key: ")?;

        app_lib::okta::store_credentials(&username, &password, &secret)?;
        let code = app_lib::okta::totp_now(&secret)?;

        println!();
        println!("{} to the macOS keychain", paint("saved", GREEN));
        println!(
            "This Mac's code right now is {} — confirm it matches your phone",
            paint(&code, GREEN)
        );
        println!("before relying on this, then run `oculus auth auto`.");
        Ok(())
    }

    /// Run the headless sign-in and report precisely why it failed, since the
    /// first real run against the live Okta policy is also the diagnosis.
    fn auth_auto(&self) -> Result<(), String> {
        match app_lib::okta::sign_in(&self.data_dir) {
            Ok(_) => {
                let name = app_lib::canvas::Canvas::open(&self.data_dir).whoami()?;
                // The app treats a missing flag as "never signed in" and will
                // not so much as probe the cookie we just wrote.
                app_lib::paths::mark_authenticated(&self.data_dir);
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            Err(e @ app_lib::okta::LoginError::BadPassword(_)) => {
                app_lib::okta::clear_password().ok();
                Err(format!("{e}\nThe stored password has been discarded — run `oculus auth setup` again."))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// One keep-alive cycle, for the LaunchAgent.
    ///
    /// Every outcome is a log line and `Ok(())`. Nothing here is an error
    /// launchd can act on: it has no console, and a non-zero exit would just be
    /// recorded as a crashed job.
    fn auth_tick(&self) -> Result<(), String> {
        use app_lib::canvas::SessionProbe;

        let log = |m: &str| app_lib::paths::append_keepalive_log(&self.data_dir, m);

        // The probe *is* the keep-alive: Canvas extends the session on use and
        // the client folds any rotated cookie back to disk on the way through.
        match app_lib::canvas::Canvas::open(&self.data_dir).probe() {
            SessionProbe::Valid(name) => {
                app_lib::paths::mark_authenticated(&self.data_dir);
                log(&format!("session extended ({name})"));
                return Ok(());
            }
            // Nothing is wrong with the session we hold — the network is down.
            // Re-authenticating here would spend an Okta attempt to learn that.
            SessionProbe::Unreachable(why) => {
                log(&format!("skipped — {why}"));
                return Ok(());
            }
            SessionProbe::Rejected(why) => log(&format!("session rejected — {why}")),
        }

        match app_lib::okta::sign_in(&self.data_dir) {
            Ok(_) => match app_lib::canvas::Canvas::open(&self.data_dir).whoami() {
                Ok(name) => {
                    app_lib::paths::mark_authenticated(&self.data_dir);
                    log(&format!("session rebuilt ({name})"));
                }
                // Signed in, but the new cookie did not verify. Leave the flag
                // alone rather than assert a session we could not confirm.
                Err(e) => log(&format!("signed in but could not verify: {e}")),
            },
            Err(e @ app_lib::okta::LoginError::BadPassword(_)) => {
                // Drop it now: replaying a wrong password every six hours,
                // unattended, is how the account gets locked.
                app_lib::okta::clear_password().ok();
                log(&format!("{e} — stored password discarded, run `oculus auth setup`"));
            }
            Err(e) => log(&format!("automated sign-in failed: {e}")),
        }
        Ok(())
    }

    fn auth_forget(&self) -> Result<(), String> {
        app_lib::okta::clear_credentials()?;
        println!("{}", paint("forgotten", YELLOW));
        Ok(())
    }

    fn auth_ed(&self, token: Option<&str>) -> Result<(), String> {
        match token {
            Some(t) => {
                let name = app_lib::ed::Ed::set_token(&self.data_dir, t)?;
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            None => {
                let ed = app_lib::ed::Ed::open(&self.data_dir);
                let name = ed.whoami()?;
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
        }
    }

    // ── auth ─────────────────────────────────────────────────────────────────

    /// Canvas sign-in is SAML through the university IdP, which needs a real
    /// browser. The app already has one, so login means: start the app, then
    /// watch for the cookie it saves. Nothing here can shortcut that.
    fn login(&self) -> Result<(), String> {
        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        if let Ok(name) = canvas.whoami() {
            println!("{} as {name}", paint("already connected", GREEN));
            return Ok(());
        }

        let cookie = app_lib::paths::cookie_path(&self.data_dir);
        let before = std::fs::metadata(&cookie).and_then(|m| m.modified()).ok();

        println!("opening Oculus for Canvas sign-in…");
        #[cfg(target_os = "macos")]
        let launched = std::process::Command::new("open").args(["-a", "Oculus"]).status().is_ok_and(|s| s.success());
        #[cfg(not(target_os = "macos"))]
        let launched = false;

        if !launched {
            println!("could not launch the app — start Oculus yourself and sign in.");
        }
        println!("waiting for the session (Ctrl-C to give up)…");

        // Poll rather than watch: the app writes the cookie from a background
        // thread two seconds after the SSO redirect lands, and a changed file
        // is the only signal that crosses process boundaries.
        for _ in 0..600 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let now = std::fs::metadata(&cookie).and_then(|m| m.modified()).ok();
            if now.is_none() || now == before {
                continue;
            }
            let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
            if let Ok(name) = canvas.whoami() {
                println!("{} as {name}", paint("connected", GREEN));
                return Ok(());
            }
        }
        Err("timed out waiting for sign-in".to_string())
    }

    fn logout(&self) -> Result<(), String> {
        let cookie = app_lib::paths::cookie_path(&self.data_dir);
        let session_dir = self.data_dir.join("canvas-session");
        let had = cookie.exists() || session_dir.exists();

        std::fs::remove_file(&cookie).ok();
        // Removes the SSO profile too, so the next login is a fresh one.
        std::fs::remove_dir_all(&session_dir).ok();

        println!("{}", if had { paint("signed out", GREEN) } else { paint("no session to clear", DIM) });
        Ok(())
    }

    // ── list ─────────────────────────────────────────────────────────────────

    fn list(&self, args: ListArgs) -> Result<(), String> {
        if args.lectures {
            return self.list_lectures(&args.codes);
        }
        self.list_subjects(args.refresh)
    }

    fn list_subjects(&self, refresh: bool) -> Result<(), String> {
        let pool = self.db();

        // Fetch live when asked, and also when the database has nothing to show
        // — a first run should not print an empty table.
        let mut rows = match &pool {
            Some(p) => self.rt.block_on(store::subjects(p))?,
            None => Vec::new(),
        };
        if refresh || rows.is_empty() {
            let engine = self.engine(false);
            if !engine.canvas.has_session() {
                return Err("not connected — run `oculus auth login`".to_string());
            }
            let courses = engine.list_courses()?;
            if let Some(p) = &pool {
                self.rt.block_on(store::upsert_subjects(p, &courses))?;
                rows = self.rt.block_on(store::subjects(p))?;
            } else {
                for c in &courses {
                    println!(
                        "{:<24} {:<12} {}",
                        c.code,
                        c.term.clone().unwrap_or_default(),
                        c.name
                    );
                }
                return Ok(());
            }
        }

        if rows.is_empty() {
            println!("{}", paint("no subjects", DIM));
            return Ok(());
        }
        for s in &rows {
            let mark = if s.is_current { paint("●", GREEN) } else { paint("○", DIM) };
            let synced = s
                .last_synced_at
                .clone()
                .map(|t| t.chars().take(10).collect::<String>())
                .unwrap_or_else(|| "never".into());
            println!(
                "{mark} {:<24} {:<14} {} {}",
                s.code,
                s.term_name.clone().unwrap_or_default(),
                paint(&format!("{synced:<11}"), DIM),
                s.name
            );
        }
        Ok(())
    }

    fn list_lectures(&self, codes: &[String]) -> Result<(), String> {
        let pool = self.db().ok_or("lectures live in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, codes, true)?;

        for s in &wanted {
            let rows = self.rt.block_on(store::lectures(&pool, s.id))?;
            println!("{}  {}", paint(&s.code, BOLD), paint(&format!("{} lectures", rows.len()), DIM));
            for l in &rows {
                let mins = l.duration_seconds / 60;
                let marks = format!(
                    "{}{}",
                    if l.has_video { "video" } else { "     " },
                    if l.has_transcript { " transcript" } else { "" }
                );
                println!(
                    "  {:<11} {mins:>4}m  {} {}",
                    l.date.chars().take(10).collect::<String>(),
                    paint(&format!("{marks:<22}"), DIM),
                    l.title
                );
            }
        }
        Ok(())
    }

    // ── run ──────────────────────────────────────────────────────────────────

    fn run(&self, args: RunArgs) -> Result<(), String> {
        if args.lectures {
            return self.run_lectures(&args);
        }
        self.run_subjects(&args)
    }

    /// Sync the Echo360 lecture list, and optionally pull the media. Each
    /// subject gets its own LTI launch — the Echo360 session is per course.
    fn run_lectures(&self, args: &RunArgs) -> Result<(), String> {
        let pool = self.db().ok_or("lectures are stored in the database")?;
        let cookie = std::fs::read_to_string(app_lib::paths::cookie_path(&self.data_dir))
            .map_err(|_| "not connected — run `oculus auth login`".to_string())?;

        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, &args.codes, !args.all)?;
        if wanted.is_empty() {
            return Err("no subjects to sync — pass subject codes or --all".to_string());
        }

        let ffmpeg = (args.videos)
            .then(|| app_lib::echo360::find_ffmpeg(None))
            .flatten();
        if args.videos && ffmpeg.is_none() {
            return Err("ffmpeg not found — run `bun run ffmpeg` in app/".to_string());
        }

        for s in &wanted {
            println!("{}", paint(&s.code, BOLD));
            let session = match app_lib::echo360::connect(cookie.trim(), s.id) {
                Ok(sess) => sess,
                Err(e) => {
                    eprintln!("  {} {e}", paint("skip", YELLOW));
                    continue;
                }
            };
            let lectures = app_lib::echo360::syllabus(&session)?;
            self.rt.block_on(store::upsert_lectures(&pool, s.id, &lectures))?;
            println!("  {} lecture(s)", lectures.len());

            for l in &lectures {
                let dir = app_lib::echo360::lecture_dir(&self.data_dir, &l.id);
                let date = l.date.chars().take(10).collect::<String>();

                if args.transcripts {
                    let path = dir.join("transcript.vtt");
                    if !path.exists() {
                        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                        match app_lib::echo360::transcript(&session, &l.lesson_id, &l.id) {
                            Ok(vtt) => {
                                std::fs::write(&path, vtt).map_err(|e| e.to_string())?;
                                self.rt.block_on(store::set_lecture_path(
                                    &pool, &l.id, "transcript_path", &path.to_string_lossy(),
                                ))?;
                                println!("  {}  {date}  {}", paint("transcript", DIM), l.title);
                            }
                            Err(e) => eprintln!("  {} {}: {e}", paint("warn", YELLOW), l.title),
                        }
                    }
                }

                if let Some(ffmpeg) = &ffmpeg {
                    let final_ = dir.join("source1.mp4");
                    if final_.exists() {
                        continue;
                    }
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let raw = dir.join("raw.mp4");
                    print!("  {}       {date}  {} ", paint("video", DIM), l.title);
                    let _ = std::io::stdout().flush();

                    let outcome = app_lib::echo360::download_url(&session, &l.id, &l.lesson_id)
                        .and_then(|url| app_lib::echo360::stream_to_file(&url, &raw, &|_| {}))
                        .and_then(|bytes| {
                            if app_lib::echo360::trim_video(ffmpeg, &raw, &final_) {
                                Ok(bytes)
                            } else {
                                Err("ffmpeg trim failed".to_string())
                            }
                        });
                    std::fs::remove_file(&raw).ok();

                    match outcome {
                        Ok(bytes) => {
                            println!("{}", paint(&human_bytes(bytes), DIM));
                            self.rt.block_on(store::set_lecture_path(
                                &pool, &l.id, "video_path", &final_.to_string_lossy(),
                            ))?;
                        }
                        Err(e) => {
                            std::fs::remove_file(&final_).ok();
                            println!("{}", paint(&e, RED));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn run_subjects(&self, args: &RunArgs) -> Result<(), String> {
        // No in-scrape parse triggering here: `index_pdfs` below is the CLI's
        // parse tier and it walks the same files serially. With both on, every
        // PDF was handed to the sidecar twice — once by the engine's queue as
        // it landed, once by `index_pdfs` — and the two could be mid-parse on
        // the same `.md`/`.pages.json` at the same time. The app keeps the
        // in-scrape trigger; it has a UI that wants progress as files arrive.
        let engine = self.engine(false);
        if !engine.canvas.has_session() {
            return Err("not connected — run `oculus auth login`".to_string());
        }
        let who = engine.canvas.whoami()?;

        let pool = self.db();

        // Refresh the course list first: a subject added this week would
        // otherwise be invisible to a code filter.
        let courses = engine.list_courses()?;
        if let Some(p) = &pool {
            self.rt.block_on(store::upsert_subjects(p, &courses))?;
        }

        let targets: Vec<sync::Subject> = match (&pool, args.codes.is_empty()) {
            // No filter and a database: honour the selection made in the app.
            (Some(p), true) => {
                let rows = self.rt.block_on(store::subjects(p))?;
                rows.iter()
                    .filter(|s| s.selected && (args.all || s.is_current))
                    .map(|s| sync::Subject { id: s.id, code: s.code.clone() })
                    .collect()
            }
            _ => courses
                .iter()
                .filter(|c| {
                    if args.codes.is_empty() {
                        args.all || c.is_current
                    } else {
                        args.codes.iter().any(|w| matches_code(&c.code, w))
                    }
                })
                .map(|c| sync::Subject { id: c.id, code: c.code.clone() })
                .collect(),
        };

        if targets.is_empty() {
            return Err(if args.codes.is_empty() {
                "no current subjects selected — pass subject codes or --all".to_string()
            } else {
                format!("no subject matched {}", args.codes.join(", "))
            });
        }

        println!("{} as {who}", paint("canvas", DIM));
        println!("syncing {} subject(s): {}", targets.len(), targets.iter().map(|s| s.code.as_str()).collect::<Vec<_>>().join(", "));
        println!();

        let target_codes: Vec<String> = targets.iter().map(|s| s.code.clone()).collect();
        let run_id = pool
            .as_ref()
            .and_then(|p| self.rt.block_on(store::start_run(p, &target_codes)).ok());

        // The engine reports through a plain trait; here that means printing a
        // line per artifact and, when there is a database, upserting the same
        // rows the app's event listener would have written.
        let reporter = TermReporter::new();
        let sink = reporter.sink();
        // The engine's fire-and-forget parse is for the app, which wants a
        // progress bar moving while it downloads. Here the index phase below
        // owns the sidecar, one PDF at a time, so the two never overlap.
        let engine = Engine::new(&self.data_dir, Box::new(reporter)).with_pdf_parsing(false);

        let started = std::time::Instant::now();
        let done = engine.scrape(&targets);

        let written = sink.lock().unwrap().clone();
        if let Some(p) = &pool {
            self.rt.block_on(async {
                for f in &written {
                    if let Err(e) = store::upsert_file(p, f.subject_id, &f.relative_path, f.size_bytes, &f.category, f.canvas_id, f.source_url.as_deref(), f.action != "unchanged").await {
                        eprintln!("{} {e}", paint("db:", YELLOW));
                    }
                }
                if let Some(id) = run_id {
                    store::finish_run(p, id, "completed", done, None).await.ok();
                    store::add_log(p, "info", &format!("CLI synced {done} subject(s)"), Some(id)).await.ok();
                }
            });
        }

        println!();
        println!(
            "{} {done} subject(s), {} artifact(s) in {:.1}s",
            paint("done", GREEN),
            written.len(),
            started.elapsed().as_secs_f64()
        );

        // Class times and due dates. Not part of the scrape: these come from
        // Canvas's calendar API and live only in the database, so there is no
        // artifact to report and nothing on disk to skip.
        if let Some(p) = &pool {
            let mut total = 0usize;
            for t in &targets {
                match app_lib::calendar::fetch(&engine.canvas, t.id) {
                    Ok(events) => {
                        total += events.len();
                        if let Err(e) =
                            self.rt.block_on(store::replace_calendar_events(p, t.id, &events))
                        {
                            eprintln!("{} {e}", paint("calendar:", YELLOW));
                        }
                    }
                    Err(e) => eprintln!("{} {}: {e}", paint("calendar:", YELLOW), t.code),
                }
            }
            println!("{} {total} calendar event(s)", paint("calendar", DIM));
        }

        if args.no_parse {
            return Ok(());
        }
        let Some(p) = &pool else {
            println!("{}", paint("no database — skipping parse and index", YELLOW));
            return Ok(());
        };

        let pdfs: Vec<(i64, String)> = written
            .iter()
            .filter(|f| f.relative_path.to_lowercase().ends_with(".pdf"))
            .map(|f| (f.subject_id, f.relative_path.clone()))
            .collect();
        self.index_pdfs(p, &pdfs, !args.no_embed)
    }

    // ── Parse + embed ────────────────────────────────────────────────────────

    /// Parse each PDF and fold it into the retrieval index.
    ///
    /// Serial by design: the sidecar has one shared heavy-work slot, so
    /// concurrent requests would queue there anyway — and one at a time is the
    /// only way the log stays readable. Both halves are idempotent, so
    /// re-running this over an already-indexed library is cheap.
    fn index_pdfs(&self, pool: &SqlitePool, pdfs: &[(i64, String)], embed: bool) -> Result<(), String> {
        if pdfs.is_empty() {
            return Ok(());
        }
        if !sidecar_healthy() {
            println!(
                "{}",
                paint("sidecar not running — PDFs left unparsed and unindexed", YELLOW)
            );
            return Ok(());
        }

        println!();
        println!(
            "{} {} PDF(s)",
            paint(if embed { "indexing" } else { "parsing" }, BOLD),
            pdfs.len()
        );

        let mut pages_total = 0usize;
        let mut failed = 0usize;

        let mut missing = 0usize;

        for (subject_id, rel) in pdfs {
            // For Office documents the parse/embed target is the derived
            // sibling PDF, not the library file itself.
            let Some(pdf_rel) = app_lib::paths::doc_pdf_rel(rel) else {
                continue;
            };
            // Rows can outlive their file — a course renamed, a library moved.
            // Those are not failures worth shouting about, just nothing to do.
            if !self.data_dir.join(&pdf_rel).is_file() {
                missing += 1;
                continue;
            }
            let name = rel.rsplit('/').next().unwrap_or(rel);
            print!("  {:<52} ", truncate(name, 52));
            let _ = std::io::stdout().flush();

            let code = rel.split('/').nth(1).unwrap_or("");
            match app_lib::sync::parse_pdf(&self.data_dir, rel, *subject_id, code, 0) {
                Ok(mode) => print!("{}", paint(&format!("{mode:<6}"), DIM)),
                Err(e) => {
                    println!("{}", paint(&e, RED));
                    failed += 1;
                    continue;
                }
            }

            if !embed {
                println!();
                continue;
            }

            let outcome = self.rt.block_on(async {
                let Some(file_id) = store::file_id(pool, *subject_id, rel).await? else {
                    return Err("not in the database".to_string());
                };
                let abs = self.data_dir.join(&pdf_rel).to_string_lossy().to_string();
                app_lib::retrieval::ingest(&app_lib::paths::db_path(&self.data_dir), file_id, abs, false, 0, String::new()).await
            });

            match outcome {
                Ok(s) => {
                    pages_total += s.pages_embedded;
                    println!(
                        "  {}",
                        paint(&format!("{} pages, {} with text", s.pages_embedded, s.pages_with_markdown), DIM)
                    );
                }
                Err(e) => {
                    println!("  {}", paint(&e, RED));
                    failed += 1;
                }
            }
        }

        self.rt
            .block_on(store::reconcile_parse_status(pool, &self.data_dir))
            .ok();

        println!();
        if embed {
            println!("{} {pages_total} page(s) indexed", paint("done", GREEN));
        }
        if failed > 0 {
            println!("{} {failed} PDF(s) failed", paint("warning", YELLOW));
        }
        if missing > 0 {
            println!(
                "{} {missing} database row(s) point at files no longer on disk",
                paint("warning", YELLOW)
            );
        }
        // The sidecar returns as soon as the fast pass has produced markdown
        // and runs the slower, better parse afterwards. That text lands on the
        // next `oculus index`.
        println!("{}", paint("quality parses continue in the sidecar", DIM));
        Ok(())
    }

    /// Re-run parse and embed over PDFs already on record — the cheap way to
    /// pick up quality-parse text without re-downloading anything.
    fn index(&self, args: &IndexArgs) -> Result<(), String> {
        let pool = self.db().ok_or("the index lives in the database")?;
        let subjects = self.rt.block_on(store::subjects(&pool))?;
        let wanted = filter_subjects(&subjects, &args.codes, false)?;
        let ids: Vec<i64> = wanted.iter().map(|s| s.id).collect();

        let pdfs = self.rt.block_on(store::pdf_files(&pool, &ids))?;
        if pdfs.is_empty() {
            println!("{}", paint("no PDFs on record — run a sync first", DIM));
            return Ok(());
        }
        self.index_pdfs(&pool, &pdfs, true)
    }
}

fn sidecar_healthy() -> bool {
    ureq::get(&format!(
        "http://127.0.0.1:{}/health",
        app_lib::sidecar::SIDECAR_PORT
    ))
    .timeout(std::time::Duration::from_millis(500))
    .call()
    .is_ok()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}

// ── Subject filtering ────────────────────────────────────────────────────────

/// `MULT20015` matches `MULT20015_2026_SM2`. Canvas codes carry a term suffix
/// nobody wants to type.
fn matches_code(code: &str, wanted: &str) -> bool {
    let (code, wanted) = (code.to_uppercase(), wanted.to_uppercase());
    code == wanted || code.starts_with(&format!("{wanted}_"))
}

fn filter_subjects(
    subjects: &[store::SubjectRow],
    codes: &[String],
    current_only_when_empty: bool,
) -> Result<Vec<store::SubjectRow>, String> {
    let picked: Vec<store::SubjectRow> = subjects
        .iter()
        .filter(|s| {
            if codes.is_empty() {
                !current_only_when_empty || s.is_current
            } else {
                codes.iter().any(|w| matches_code(&s.code, w))
            }
        })
        .cloned()
        .collect();

    if picked.is_empty() && !codes.is_empty() {
        return Err(format!("no subject matched {}", codes.join(", ")));
    }
    Ok(picked)
}

// ── Terminal reporter ────────────────────────────────────────────────────────

/// Prints one line per artifact and remembers them so the caller can write the
/// database in one pass at the end.
struct TermReporter {
    phase: Mutex<String>,
    counter: Mutex<(usize, usize)>,
    written: std::sync::Arc<Mutex<Vec<FileEvent>>>,
    course: Mutex<String>,
}

impl TermReporter {
    fn new() -> Self {
        TermReporter {
            phase: Mutex::new(String::new()),
            counter: Mutex::new((0, 0)),
            written: Default::default(),
            course: Mutex::new(String::new()),
        }
    }
    fn sink(&self) -> std::sync::Arc<Mutex<Vec<FileEvent>>> {
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
        let short = f.relative_path.splitn(3, '/').nth(2).unwrap_or(&f.relative_path);
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

    fn log(&self, level: &str, course: &str, message: &str) {
        let tag = match level {
            "error" => paint("error", RED),
            "warning" => paint("warn", YELLOW),
            _ => paint("info", DIM),
        };
        eprintln!("  {tag} {course}: {message}");
    }
}

fn human_bytes(n: u64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    } else if n >= 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{n} B")
    }
}

// ── Colour ───────────────────────────────────────────────────────────────────

const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";

fn colour_ok() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal())
}

fn paint(s: &str, code: &str) -> String {
    if s.is_empty() || !colour_ok() {
        s.to_string()
    } else {
        format!("{code}{s}\x1b[0m")
    }
}
