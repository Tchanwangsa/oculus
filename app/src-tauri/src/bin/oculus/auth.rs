//! `oculus status` and `oculus auth`.

use super::*;

impl Ctx {
    // ── status ───────────────────────────────────────────────────────────────

    pub(crate) fn status(&self) -> Result<(), String> {
        #[derive(Serialize)]
        struct Service {
            connected: bool,
            user: Option<String>,
            /// Why not, when `connected` is false.
            detail: Option<String>,
        }
        /// The configured parse backend's own `Health`. `parser_version` is the
        /// version handshake for artifacts this binary reads.
        #[derive(Serialize)]
        struct ParserStatus {
            backend: String,
            ready: bool,
            parser_version: Option<u32>,
            detail: Option<String>,
        }
        #[derive(Serialize)]
        struct Counts {
            total: usize,
            current: usize,
            synced: usize,
        }
        #[derive(Serialize)]
        struct FileCounts {
            total: i64,
            parsed: i64,
        }
        fn describe(s: &Service) -> String {
            match (&s.user, &s.detail) {
                (Some(name), _) => format!("{} as {name}", paint("connected", GREEN)),
                (None, Some(why)) => paint(why, YELLOW),
                (None, None) => paint("unknown", DIM),
            }
        }

        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        let canvas_status = match canvas.whoami() {
            Ok(name) => Service {
                connected: true,
                user: Some(name),
                detail: None,
            },
            Err(_) if !canvas.has_session() => Service {
                connected: false,
                user: None,
                detail: Some("signed out — run `oculus auth login`".to_string()),
            },
            Err(e) => Service {
                connected: false,
                user: None,
                detail: Some(e),
            },
        };

        let ed = app_lib::ed::Ed::open(&self.data_dir);
        let ed_status = if !ed.has_session() {
            Service {
                connected: false,
                user: None,
                detail: Some("not connected — connects automatically on the next sync".to_string()),
            }
        } else {
            match ed.whoami() {
                Ok(name) => Service {
                    connected: true,
                    user: Some(name),
                    detail: None,
                },
                Err(e) => Service {
                    connected: false,
                    user: None,
                    detail: Some(e),
                },
            }
        };

        // Local only, no quota; the local engine's `health()` is a loopback connect
        // with its own short timeout.
        let parser = match app_lib::parse::backend() {
            Ok(backend) => match app_lib::parse::preflight(backend.as_ref()) {
                Ok(health) => ParserStatus {
                    backend: health.backend,
                    ready: health.ready,
                    parser_version: Some(health.parser_version),
                    detail: None,
                },
                Err(e) => ParserStatus {
                    backend: backend.health().backend,
                    ready: false,
                    parser_version: Some(backend.health().parser_version),
                    detail: Some(e.to_string()),
                },
            },
            Err(e) => ParserStatus {
                backend: app_lib::parse::parse_config().engine.as_str().to_string(),
                ready: false,
                parser_version: None,
                detail: Some(e.to_string()),
            },
        };

        let pool = self.db();
        let (subjects, files) = match &pool {
            Some(pool) => self.rt.block_on(async {
                let rows = store::subjects(pool).await.unwrap_or_default();
                let counts = Counts {
                    total: rows.len(),
                    current: rows.iter().filter(|s| s.is_current).count(),
                    synced: rows.iter().filter(|s| s.last_synced_at.is_some()).count(),
                };
                let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM files")
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);
                let parsed: i64 =
                    sqlx::query_scalar("SELECT COUNT(*) FROM files WHERE parse_status = 'quality'")
                        .fetch_one(pool)
                        .await
                        .unwrap_or(0);
                (Some(counts), Some(FileCounts { total, parsed }))
            }),
            None => (None, None),
        };

        let index = pool.as_ref().and_then(|_| {
            self.rt
                .block_on(app_lib::retrieval::stats(&app_lib::paths::db_path(
                    &self.data_dir,
                )))
                .ok()
        });

        if self.json {
            #[derive(Serialize)]
            struct Report<'a> {
                data_dir: String,
                canvas: &'a Service,
                ed: &'a Service,
                parser: &'a ParserStatus,
                subjects: &'a Option<Counts>,
                files: &'a Option<FileCounts>,
                index: &'a Option<app_lib::retrieval::IndexStats>,
            }
            return self.emit(&Report {
                data_dir: self.data_dir.display().to_string(),
                canvas: &canvas_status,
                ed: &ed_status,
                parser: &parser,
                subjects: &subjects,
                files: &files,
                index: &index,
            });
        }

        println!("{}  {}", paint("library", DIM), self.data_dir.display());
        println!("{}   {}", paint("canvas", DIM), describe(&canvas_status));
        println!("{}       {}", paint("ed", DIM), describe(&ed_status));
        println!(
            "{}   {}",
            paint("parser", DIM),
            match (&parser.detail, parser.parser_version) {
                (None, Some(v)) => paint(&format!("{} (v{v})", parser.backend), GREEN),
                (Some(why), _) => paint(why, YELLOW),
                (None, None) => paint("unknown", DIM),
            }
        );
        if let Some(c) = &subjects {
            println!(
                "{} {} ({} current, {} synced)",
                paint("subjects", DIM),
                c.total,
                c.current,
                c.synced
            );
        }
        if let Some(f) = &files {
            println!(
                "{}    {} ({} parsed)",
                paint("files", DIM),
                f.total,
                f.parsed
            );
        }
        if let Some(i) = &index {
            println!(
                "{}    {} page(s) across {} file(s){}",
                paint("index", DIM),
                i.pages_embedded,
                i.files_embedded,
                match &i.model {
                    Some(m) => paint(&format!("  {m}"), DIM),
                    None => String::new(),
                }
            );
            // Stored but not searchable: its own line so the searchable count stays plain.
            if i.pages_stale > 0 {
                println!(
                    "{}    {} page(s) need re-embedding{}",
                    paint("stale", DIM),
                    i.pages_stale,
                    match i.stale_models.is_empty() {
                        true => String::new(),
                        false => paint(&format!("  from {}", i.stale_models.join(", ")), DIM),
                    }
                );
            }
        }
        Ok(())
    }

    pub(crate) fn auth_status(&self) -> Result<(), String> {
        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        match canvas.whoami() {
            Ok(name) => {
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Walk the user through storing credentials. The setup key cannot be
    /// recovered later, so the instructions matter as much as the prompts.
    pub(crate) fn auth_setup(&self) -> Result<(), String> {
        println!("{}", paint("Automated sign-in setup", DIM));
        println!();
        println!("This needs a TOTP factor enrolled and its setup key. A TOTP code is a");
        println!("one-way function of a secret seed, so it cannot be worked out from other");
        println!("codes — the seed is shown only at enrolment. If you never copied it:");
        println!();
        println!("  1. Open https://sso.unimelb.edu.au/enduser/settings");
        println!("  2. Google Authenticator → remove it, then set it up again");
        println!("  3. On the QR page click \"Can't scan?\" to reveal the setup key");
        println!(
            "  4. {} scan the QR with your phone as well, so you keep",
            paint("Also", YELLOW)
        );
        println!("     a working authenticator if this Mac is ever unavailable.");
        println!();

        let username = read_line("Username (e.g. chanwangsat): ")?;
        let password = read_secret("Password: ")?;
        let secret = read_secret("Authenticator setup key: ")?;

        app_lib::okta::store_credentials(&username, &password, &secret)?;
        let code = app_lib::okta::totp_now(&secret)?;

        println!();
        println!("{}", paint("saved", GREEN));
        println!(
            "This Mac's code right now is {} — confirm it matches your phone",
            paint(&code, GREEN)
        );
        println!("before relying on this, then run `oculus auth auto`.");
        Ok(())
    }

    /// Run the headless sign-in and report precisely why it failed.
    pub(crate) fn auth_auto(&self) -> Result<(), String> {
        match app_lib::okta::sign_in(
            &self.data_dir,
            app_lib::okta::Trigger::Manual,
            app_lib::okta::Role::Cli,
        ) {
            Ok(_) => {
                let name = app_lib::canvas::Canvas::open(&self.data_dir).whoami()?;
                // Without the flag the app treats this as never signed in.
                app_lib::paths::mark_authenticated(&self.data_dir);
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            Err(e @ app_lib::okta::LoginError::BadPassword(_)) => Err(format!(
                "{e}\nThe stored password has been discarded — run `oculus auth setup` again."
            )),
            Err(e) => Err(e.to_string()),
        }
    }

    /// One keep-alive cycle, for the LaunchAgent.
    ///
    /// Every outcome is a log line and `Ok(())`: launchd would only record a
    /// non-zero exit as a crashed job.
    pub(crate) fn auth_tick(&self) -> Result<(), String> {
        use app_lib::canvas::SessionProbe;

        let log = |m: &str| app_lib::paths::append_keepalive_log(&self.data_dir, m);

        if app_lib::paths::signed_out_path(&self.data_dir).exists() {
            log("skipped — signed out");
            return Ok(());
        }

        // The probe is the keep-alive: Canvas extends the session on use.
        match app_lib::canvas::Canvas::open(&self.data_dir).probe() {
            SessionProbe::Valid(name) => {
                app_lib::paths::mark_authenticated(&self.data_dir);
                log(&format!("session extended ({name})"));
                return Ok(());
            }
            // The network is down; re-authenticating would waste an Okta attempt.
            SessionProbe::Unreachable(why) => {
                log(&format!("skipped — {why}"));
                return Ok(());
            }
            SessionProbe::Rejected(why) => log(&format!("session rejected — {why}")),
        }

        match app_lib::okta::sign_in(
            &self.data_dir,
            app_lib::okta::Trigger::KeepAlive,
            app_lib::okta::Role::Cli,
        ) {
            Ok(_) => match app_lib::canvas::Canvas::open(&self.data_dir).whoami() {
                Ok(name) => {
                    app_lib::paths::mark_authenticated(&self.data_dir);
                    log(&format!("session rebuilt ({name})"));
                }
                // Leave the flag alone rather than assert an unverified session.
                Err(e) => log(&format!("signed in but could not verify: {e}")),
            },
            Err(e @ app_lib::okta::LoginError::BadPassword(_)) => {
                log(&format!(
                    "{e} — stored password discarded, run `oculus auth setup`"
                ));
            }
            Err(
                e @ (app_lib::okta::LoginError::Waiting(_) | app_lib::okta::LoginError::Paused(_)),
            ) => {
                log(&format!("automated sign-in skipped: {e}"));
            }
            Err(e) => log(&format!("automated sign-in failed: {e}")),
        }
        Ok(())
    }

    pub(crate) fn auth_forget(&self) -> Result<(), String> {
        app_lib::okta::clear_credentials()?;
        println!("{}", paint("forgotten", YELLOW));
        Ok(())
    }

    pub(crate) fn auth_ed(&self, token: Option<&str>) -> Result<(), String> {
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

    /// Canvas sign-in is SAML and needs a real browser: start the app and
    /// watch for the cookie it saves.
    pub(crate) fn login(&self) -> Result<(), String> {
        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        if let Ok(name) = canvas.whoami() {
            println!("{} as {name}", paint("already connected", GREEN));
            return Ok(());
        }

        let cookie = app_lib::paths::cookie_path(&self.data_dir);
        let before = std::fs::metadata(&cookie).and_then(|m| m.modified()).ok();

        println!("opening Oculus for Canvas sign-in…");
        #[cfg(target_os = "macos")]
        let launched = std::process::Command::new("open")
            .args(["-a", "Oculus"])
            .status()
            .is_ok_and(|s| s.success());
        #[cfg(not(target_os = "macos"))]
        let launched = false;

        if !launched {
            println!("could not launch the app — start Oculus yourself and sign in.");
        }
        println!("waiting for the session (Ctrl-C to give up)…");

        // Poll: a changed cookie file is the only signal that crosses processes.
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

    pub(crate) fn logout(&self) -> Result<(), String> {
        // Drops the Okta session too, so the next login is a fresh one.
        let had = app_lib::paths::sign_out(&self.data_dir).map_err(|e| e.to_string())?;

        println!(
            "{}",
            if had {
                paint("signed out", GREEN)
            } else {
                paint("no session to clear", DIM)
            }
        );
        Ok(())
    }
}
