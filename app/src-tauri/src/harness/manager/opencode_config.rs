//! opencode's `agents/opencode.json`, rendered with this library's brief,
//! paths and Studio subagents.

use std::path::{Path, PathBuf};

use crate::harness::opencode;
use crate::harness::Provider;

use super::instructions::{instructions, thread_cwd};
use super::naming::NAMING_INSTRUCTIONS;

/// Write the config the shared opencode server starts from. Rendered before
/// every server start, and by the library move, whose paths it names.
pub fn write_opencode_config(data_dir: &Path) -> Result<PathBuf, String> {
    opencode::write_config(
        &thread_cwd(data_dir),
        data_dir,
        &instructions(data_dir, None, None, None),
        &opencode::OneOffPrompts {
            naming: NAMING_INSTRUCTIONS,
            writer: crate::harness::suggest::INSTRUCTIONS,
            lecture_end: crate::lectures::lecture_end::INSTRUCTIONS,
        },
        &crate::harness::studio_agents::opencode_agents_fragment(
            &crate::harness::studio_agents::assigned_now(Provider::Opencode),
        ),
    )
}
