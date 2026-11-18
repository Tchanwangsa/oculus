//! Arguments of `oculus library`.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum LibraryAction {
    Migrate(LibraryMigrateArgs),
}

/// Move an old library into the Drive tree, or show what the move would do
///
/// Every command (and the app) runs this move first when it finds the old
/// layout, so running it by hand is only needed to watch it or preview it.
/// Synced folders go into each subject's Course/ folder, the student's own
/// folders flatten into the subject's root, lecture recordings move from
/// lectures/<id>/ into Course/Recordings/<date>-<slug>/, parse records into
/// .oculus/derived/, and the database's paths are rewritten to match. Old
/// paths stay readable through the path_moves table. Then each drawing saved
/// as .canvas becomes .whiteboard, with its rows and old paths following.
/// Once done, markers in the data folder's .library-move/ make both no-ops.
#[derive(Args)]
pub(crate) struct LibraryMigrateArgs {
    /// Print the plan (counts per kind and the first moves) without moving,
    /// writing or locking anything
    #[arg(long)]
    pub(crate) dry_run: bool,
    /// How many moves the dry run lists
    #[arg(long, default_value_t = 40)]
    pub(crate) show: usize,
}
