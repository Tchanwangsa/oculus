//! Opening a model as an ONNX Runtime session on the CPU.

use std::path::Path;

use ort::session::Session;

/// Threads one inference may use: half the cores, at most 4. Past 4 the
/// aligner gains little (3.8 s at 4 threads, 2.9 s at 8 on a 57 s clip), and
/// the rest of the app keeps its cores.
pub fn intra_threads() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    (cores / 2).clamp(1, 4)
}

/// A session over the model at `path`, on the CPU execution provider only.
/// Naming the CPU provider replaces ort's default policy, which prefers an
/// NPU (CoreML on a Mac). The default arena stays on: turning it off raised
/// peak memory at the same speed.
pub fn session(path: &Path) -> Result<Session, String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    Session::builder()
        .map_err(|e| fail(&e))?
        .with_intra_threads(intra_threads())
        .map_err(|e| fail(&e))?
        .with_execution_providers([ort::ep::CPU::default().build()])
        .map_err(|e| fail(&e))?
        .commit_from_file(path)
        .map_err(|e| fail(&e))
}
