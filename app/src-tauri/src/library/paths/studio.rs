use std::path::{Path, PathBuf};

/// `<data dir>/studio`: Studio's videos and its component and theme
/// libraries (docs/studio.md). It holds no secrets.
pub fn studio_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("studio")
}

/// The folders an agent thread may write outside its `agents/` cwd, beside
/// the database's files (`db_write_paths`). Each harness renders this list
/// into its own grant (docs/harness.md); the manager creates them first,
/// since Codex fails a turn on a writable root it cannot stat.
pub fn agent_write_dirs(data_dir: &Path) -> Vec<PathBuf> {
    vec![studio_dir(data_dir)]
}
