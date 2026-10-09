//! `oculus` — the Oculus command line.
//!
//! Same engine the app runs, without the window: it reads the session cookie
//! and the database the app already maintains, so a CLI sync and an in-app sync
//! are the same operation and either can follow the other.

use std::collections::HashMap;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use app_lib::agents;
use app_lib::db::projects;
use app_lib::db::store;
use app_lib::library::paths;
use app_lib::sync::{self, Engine, FileEvent, Progress, Reporter};
use clap::{Args, CommandFactory, Parser, Subcommand};
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use tokio::runtime::Runtime;

mod args;
mod commands;
mod output;

use args::*;
use commands::query::*;
use output::*;

fn main() {
    restore_sigpipe();
    let cli = Cli::parse();
    let ctx = Ctx::new(cli.json);

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
                print!("{}", app_lib::auth::okta::diagnose());
                Ok(())
            }
            AuthAction::Ed { token } => ctx.auth_ed(token.as_deref()),
        },
        Some(Command::List(args)) => ctx.list(args),
        Some(Command::Run(args)) => ctx.run(args),
        Some(Command::Index(args)) => ctx.index(&args),
        Some(Command::Search(args)) => ctx.search(&args),
        Some(Command::Grep(args)) => ctx.grep(&args),
        Some(Command::Read(args)) => ctx.read(&args),
        Some(Command::Files(args)) => ctx.files(&args),
        Some(Command::Calendar(args)) => ctx.calendar(&args),
        Some(Command::Project { action }) => match action {
            ProjectAction::List(a) => ctx.project_list(&a),
            ProjectAction::Show(a) => ctx.project_show(&a),
            ProjectAction::Create(a) => ctx.project_create(&a),
            ProjectAction::Update(a) => ctx.project_update(&a),
        },
        Some(Command::Task { action }) => match action {
            TaskAction::List(a) => ctx.task_list(&a),
            TaskAction::Add(a) => ctx.task_add(&a),
            TaskAction::Update(a) => ctx.task_update(&a),
            TaskAction::Move(a) => ctx.task_move(&a),
            TaskAction::Refile(a) => ctx.task_refile(&a),
            TaskAction::Rm(a) => ctx.task_rm(&a),
        },
        Some(Command::Lecture { action }) => match action {
            LectureAction::Candidates(a) => ctx.lecture_candidates(&a),
            LectureAction::Chapters(a) => ctx.lecture_chapters(&a),
            LectureAction::End(a) => ctx.lecture_end(&a),
        },
        Some(Command::Memory { action }) => match action {
            MemoryAction::List(a) => ctx.memory_list(&a),
            MemoryAction::Read(a) => ctx.memory_read(&a),
            MemoryAction::Write(a) => ctx.memory_write(&a),
            MemoryAction::Rm(a) => ctx.memory_rm(&a),
            MemoryAction::Move(a) => ctx.memory_move(&a),
            MemoryAction::Reindex(a) => ctx.memory_reindex(&a),
            MemoryAction::Promote(a) => ctx.memory_promote(&a),
        },
        Some(Command::Transcribe(args)) => ctx.transcribe(&args),
        Some(Command::Docs(args)) => ctx.docs(&args),
        Some(Command::Agent(args)) => ctx.agent(&args),
    };

    if let Err(e) = result {
        if cli.json {
            eprintln!("{}", serde_json::json!({ "error": e }));
        } else {
            eprintln!("{} {e}", paint("error:", RED));
        }
        std::process::exit(1);
    }
}

fn read_line(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line.trim().to_string())
}

/// `-` reads stdin; a path reads UTF-8, with the command's name on stdin errors.
fn read_input(source: &str, what: &str) -> Result<String, String> {
    if source != "-" {
        return std::fs::read_to_string(source).map_err(|e| format!("reading {source}: {e}"));
    }
    let mut buffer = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer)
        .map_err(|e| format!("reading the {what} from stdin: {e}"))?;
    Ok(buffer)
}

/// Prompt without echoing; echo is restored even if the read fails.
fn read_secret(prompt: &str) -> Result<String, String> {
    use std::io::Write;
    print!("{prompt}");
    std::io::stdout().flush().ok();

    let hidden = std::process::Command::new("stty")
        .arg("-echo")
        .status()
        .is_ok();
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

/// Rust ignores SIGPIPE, which turns `oculus list | head` into a panic.
fn restore_sigpipe() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// What every command shares: the data directory, the async runtime and
/// whether to print JSON.
struct Ctx {
    data_dir: PathBuf,
    rt: Runtime,
    json: bool,
}

impl Ctx {
    fn new(json: bool) -> Self {
        Ctx {
            data_dir: app_lib::library::paths::data_dir(),
            rt: Runtime::new().expect("tokio runtime"),
            json,
        }
    }

    /// One pretty-printed JSON document on stdout.
    fn emit(&self, value: &impl Serialize) -> Result<(), String> {
        let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        println!("{text}");
        Ok(())
    }

    fn engine(&self, parse: bool) -> Engine {
        Engine::new(&self.data_dir, Box::new(TermReporter::new())).with_pdf_parsing(parse)
    }

    /// The database, or `None` with a warning: scraping works without it.
    fn db(&self) -> Option<SqlitePool> {
        match self.rt.block_on(store::open(&self.data_dir)) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("{} {e}", paint("warning:", YELLOW));
                None
            }
        }
    }
}
