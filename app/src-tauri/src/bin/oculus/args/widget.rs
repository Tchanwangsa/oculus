//! Arguments of `oculus widget`.

use crate::*;

#[derive(Subcommand)]
pub(crate) enum WidgetAction {
    Check(WidgetCheckArgs),
}

/// Check a chat widget's TSX before it is sent
///
/// First compiles it the way the chat and `oculus artifact save --format
/// widget` do: TypeScript and JSX, the default export, and imports only
/// from react, oculus/ui, oculus/state, oculus/2d, oculus/math, oculus/3d,
/// oculus/ask, three and @react-three/fiber. A widget that doesn't compile
/// gets that one finding.
///
/// Then lints the code it parsed, each finding with its rule, line and
/// column:
///
/// title: the first line isn't `// title: …`.
///
/// colour: a hex colour (#rgb, #rgba, #rrggbb, #rrggbbaa) or rgb(), rgba(),
/// hsl(), hsla(), hwb(), lab(), lch(), oklab() or oklch() in a string, or a
/// 0xRRGGBB number. Name a theme token instead.
///
/// font: a fontFamily or font-family key or attribute, or `font-family:` in
/// a string.
///
/// tex: a JSX attribute string holding \\, which stays two backslashes;
/// write tex={"\\theta"}.
///
/// sandbox: a global fetch, XMLHttpRequest, WebSocket, EventSource,
/// localStorage, sessionStorage, indexedDB, eval, Function or
/// document.cookie, bare or on window, globalThis or self.
///
/// cleanup: setInterval, setTimeout or requestAnimationFrame (global), or
/// any .addEventListener, outside the callback of a useEffect or
/// useLayoutEffect, or in one whose returned cleanup doesn't call the
/// matching clearInterval, clearTimeout, cancelAnimationFrame or
/// removeEventListener.
///
/// A name the widget declares itself is its own and passes. Any finding
/// exits 1. `--json` prints them as a list of `{rule, line, col,
/// message}` (`line` and `col` are null for a compile error with no place).
///
///     oculus widget check outputs/bubble-sort.tsx
#[derive(Args)]
pub(crate) struct WidgetCheckArgs {
    /// The widget's TSX file, or - for stdin
    #[arg(value_name = "FILE|-")]
    pub(crate) file: String,
}
