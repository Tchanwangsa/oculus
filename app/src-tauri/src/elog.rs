//! `elog!`, the crate's `eprintln!`. `eprintln!` panics when stderr is a
//! closed pipe, which is what an app orphaned by a dead `tauri dev` writes
//! to, and a panic inside an Objective-C callback (a `with_webview` closure,
//! a wry `on_*` handler, a block WebKit calls) cannot unwind and aborts the
//! app. `elog!` drops the write error instead. Tests keep `eprintln!`, which
//! the test harness captures and a raw `stderr()` write bypasses.

macro_rules! elog {
    ($($arg:tt)*) => {{
        #[cfg(not(test))]
        {
            use std::io::Write as _;
            let _ = writeln!(std::io::stderr(), $($arg)*);
        }
        #[cfg(test)]
        eprintln!($($arg)*);
    }};
}
