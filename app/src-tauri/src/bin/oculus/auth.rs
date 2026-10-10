//! `oculus auth`.

use super::*;

impl Ctx {
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
        match app_lib::okta::sign_in(&self.data_dir, app_lib::okta::Trigger::Manual) {
            Ok(_) => {
                let name = app_lib::canvas::Canvas::open(&self.data_dir).whoami()?;
                println!("{} as {name}", paint("connected", GREEN));
                Ok(())
            }
            Err(e @ app_lib::okta::LoginError::BadPassword(_)) => Err(format!(
                "{e}\nThe stored password has been discarded — run `oculus auth setup` again."
            )),
            Err(e) => Err(e.to_string()),
        }
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

    /// Canvas sign-in is SAML and needs a real browser: start the app and
    /// watch oculus-keyd for the session it stores.
    pub(crate) fn login(&self) -> Result<(), String> {
        let canvas = app_lib::canvas::Canvas::open(&self.data_dir);
        if let Ok(name) = canvas.whoami() {
            println!("{} as {name}", paint("already connected", GREEN));
            return Ok(());
        }

        let mut watch = app_lib::auth::SignInWatch::begin(&self.data_dir)?;

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

        // Poll: what oculus-keyd reports is the only signal that crosses processes.
        for _ in 0..600 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if !watch.poll() {
                continue;
            }
            match app_lib::canvas::Canvas::open(&self.data_dir).whoami() {
                Ok(name) => {
                    println!("{} as {name}", paint("connected", GREEN));
                    return Ok(());
                }
                Err(_) => watch.declined(),
            }
        }
        Err("timed out waiting for sign-in".to_string())
    }

    pub(crate) fn logout(&self) -> Result<(), String> {
        // Drops the Okta session too, so the next login is a fresh one.
        let had = app_lib::auth::sign_out(&self.data_dir)?;

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
