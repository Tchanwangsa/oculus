use std::path::{Path, PathBuf};

/// The registration that starts keyd on a connect: the program it runs and
/// whether the OS has it loaded now.
#[cfg(feature = "client")]
#[derive(Debug, Clone)]
pub struct Registration {
    /// Where the registration lives (a file, on every OS so far).
    pub path: PathBuf,
    /// The keyd it runs, when one is registered.
    pub program: Option<String>,
    pub loaded: bool,
}

#[cfg(feature = "client")]
pub trait Registrar: Sync {
    /// Refuses, before anything is written, a `data_dir` keyd cannot serve
    /// from here.
    fn check(&self, data_dir: &Path) -> Result<(), String>;
    /// Whether `program` must be registered where it is (inside an install,
    /// for the caller check) rather than copied, with its helper app, to
    /// `paths::installed_bin`.
    fn runs_in_place(&self, program: &Path) -> bool;
    /// Registers `program` to serve `data_dir`, replacing any registration
    /// and leaving it loaded. Returns `Registration::path`.
    fn install(&self, program: &Path, data_dir: &Path) -> Result<PathBuf, String>;
    fn status(&self) -> Result<Registration, String>;
    /// Unloads keyd and removes the registration; returns what it removed.
    fn uninstall(&self) -> Result<Vec<PathBuf>, String>;
    /// Unloads and removes the registration named `label`, one an earlier
    /// Oculus made for something else; returns what it removed, nothing when
    /// there is none. Refuses keyd's own label.
    fn retire(&self, label: &str) -> Result<Vec<PathBuf>, String>;
}

#[cfg(feature = "client")]
pub fn registrar() -> &'static dyn Registrar {
    super::imp::registrar()
}
