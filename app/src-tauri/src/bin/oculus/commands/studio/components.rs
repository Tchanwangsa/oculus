//! `oculus studio components`: the components a video can use, read through
//! the library cascade, and seeding the global library with the starter set
//! (docs/studio.md, "Libraries cascade").

use super::studio_error;
use crate::*;
use app_lib::studio::libraries;
use studio_core::ComponentSource;

impl Ctx {
    pub(crate) fn studio_components(&self, action: &StudioComponentsAction) -> Result<(), String> {
        match action {
            StudioComponentsAction::List(args) => {
                let project =
                    libraries::open(&args.dir, args.subject.clone(), None).map_err(studio_error)?;
                let levels = project.components.levels();
                if self.json {
                    let rows: Vec<serde_json::Value> = levels
                        .iter()
                        .map(|(name, level)| serde_json::json!({ "name": name, "level": level }))
                        .collect();
                    return self.emit(&rows);
                }
                if levels.is_empty() {
                    println!("{}", paint("no components in any library", DIM));
                }
                for (name, level) in &levels {
                    println!("{name:<24} {}", paint(level, DIM));
                }
                Ok(())
            }
            StudioComponentsAction::Show(args) => {
                let project =
                    libraries::open(&args.dir, args.subject.clone(), None).map_err(studio_error)?;
                let level = project
                    .components
                    .levels()
                    .into_iter()
                    .find(|(name, _)| *name == args.component)
                    .map(|(_, level)| level);
                // A miss is the cascade's own error, listing every name.
                let source = project
                    .components
                    .read(&args.component)
                    .map_err(studio_error)?;
                let level = level.unwrap_or_default();
                if self.json {
                    return self.emit(&serde_json::json!({
                        "name": args.component,
                        "level": level,
                        "source": source,
                    }));
                }
                println!("{}", paint(&format!("{} · {level}", args.component), DIM));
                print!("{source}");
                Ok(())
            }
            StudioComponentsAction::Seed { reset } => {
                let global = libraries::global_dir();
                let written = studio_core::seed_library(&global, *reset).map_err(studio_error)?;
                if self.json {
                    return self.emit(&serde_json::json!({
                        "dir": global.to_string_lossy(),
                        "written": written,
                    }));
                }
                if written.is_empty() {
                    println!(
                        "{}",
                        paint(&format!("nothing to copy into {}", global.display()), DIM)
                    );
                }
                for rel in &written {
                    println!("{} {}", paint("wrote", GREEN), global.join(rel).display());
                }
                Ok(())
            }
        }
    }
}
