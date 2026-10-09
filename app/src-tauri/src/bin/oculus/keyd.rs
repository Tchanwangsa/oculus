//! `oculus keyd`: install, inspect and remove the credential broker. The
//! work is `app_lib::keyd`, which the app's startup shares.

use super::*;
use app_lib::keyd;

fn short(hash: &str) -> &str {
    &hash[..hash.len().min(12)]
}

impl Ctx {
    pub(crate) fn keyd_install(&self, from: Option<&std::path::Path>, if_changed: bool) -> Result<(), String> {
        let from = match from {
            Some(p) => p.to_path_buf(),
            None => keyd::candidate().ok_or("no built oculus-keyd found — run `bun run keyd`, or pass --from")?,
        };
        let installed = if if_changed {
            keyd::install_if_changed(&self.data_dir, &from)?
        } else {
            Some(keyd::install(&self.data_dir, &from)?)
        };
        if self.json {
            return self.emit(&installed);
        }
        let Some(installed) = installed else {
            println!("{}", paint("the installed keyd is this source — nothing to install", DIM));
            return Ok(());
        };
        println!("{} source {}", paint("installed", GREEN), short(&installed.source_hash));
        println!("  program  {}", installed.program.display());
        println!("  agent    {}", installed.plist.display());
        Ok(())
    }

    pub(crate) fn keyd_status(&self) -> Result<(), String> {
        let s = keyd::status(&self.data_dir)?;
        if self.json {
            return self.emit(&s);
        }

        match &s.program {
            Some(p) => println!("agent      {} → {p}", s.plist.display()),
            None => println!("agent      {}", paint("not installed — run `oculus keyd install`", YELLOW)),
        }
        println!("launchd    {}", if s.loaded { paint("loaded", GREEN) } else { paint("not loaded", YELLOW) });
        match &s.installed_hash {
            Some(h) => println!("installed  source {}", short(h)),
            None => println!("installed  {}", paint("no stamp", DIM)),
        }
        match (&s.candidate, &s.candidate_hash, &s.candidate_error) {
            (Some(c), Some(h), _) => {
                let verdict = if s.installed_hash.as_deref() == Some(h.as_str()) {
                    paint("same as installed", DIM)
                } else {
                    paint("differs from installed — `oculus keyd install` takes it", YELLOW)
                };
                println!("available  source {} {verdict}", short(h));
                println!("           {}", c.display());
            }
            (Some(c), None, Some(e)) => println!("available  {} {}", c.display(), paint(e, RED)),
            _ => println!("available  {}", paint("no built keyd beside this binary", DIM)),
        }
        match (&s.ping, &s.ping_error) {
            (Some(p), _) => {
                let hash = p["source_hash"].as_str().unwrap_or("?");
                println!(
                    "keyd       {} pid {}, version {}, source {}",
                    paint("answering", GREEN),
                    p["pid"],
                    p["version"].as_str().unwrap_or("?"),
                    short(hash)
                );
                if s.installed_hash.as_deref().is_some_and(|h| h != hash) {
                    println!("           {}", paint("the running keyd is not the installed one", YELLOW));
                }
            }
            (None, Some(e)) => println!("keyd       {}", paint(&format!("unreachable: {e}"), YELLOW)),
            _ => {}
        }
        match s.vault_bytes {
            Some(n) => println!("vault      {} ({})", s.vault.display(), human_bytes(n)),
            None => println!("vault      {}", paint("none yet", DIM)),
        }
        Ok(())
    }

    pub(crate) fn keyd_uninstall(&self) -> Result<(), String> {
        let removed = keyd::uninstall(&self.data_dir)?;
        if self.json {
            return self.emit(&removed);
        }
        if removed.is_empty() {
            println!("{}", paint("nothing installed", DIM));
        }
        for p in removed {
            println!("{} {}", paint("removed", GREEN), p.display());
        }
        println!("The vault and the keychain's master key are kept.");
        Ok(())
    }
}
