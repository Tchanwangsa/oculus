# Shared desktop validation

## Repository consolidation — 2026-09-28

The `andre` integration branch combines macOS `98df2e0` with the Windows
port `778d9e3`, preserving both histories. `master` remains the release/review
baseline until the integration is accepted. The previous `oculus_window`
repository is retired; shared work and platform fixes belong here.

The merge keeps the latest per-tab conversations, Claude's CLI model
catalogue, selectable opencode models and Antigravity's managed permission
rules. It also keeps Windows WSL2 containment, native browser focus, physical
SQLite path handling, sync persistence and local parsing defaults. Native
macOS commands, Keychain, WebKit and Command shortcuts remain platform gated.
All 37 registered database migrations are unchanged from the macOS baseline.

## Local checks

Checks ran on Windows from this combined checkout, with Bun 1.4.2 and the
Rust MSVC toolchain. Logs under `artifacts/` are local and ignored.

| Check | Result |
| --- | --- |
| Frontend | 44 tests, 234 assertions; TypeScript and Vite production build pass. `artifacts/unified-frontend-tests.log`. |
| Rust release suite | 494 passed, four ignored: 477 library, 14 CLI, one isolated credential test, two database-upgrade tests. `artifacts/unified-rust-tests.log`. |
| WSL2 bridge | 22 tests passed, including model-catalogue lifecycle and initialization without a model turn. |
| Installed Linux Claude | The initialization-only model query returned five catalogue entries without sending a prompt. |
| Documentation and packaging scripts | No broken source citations; all JavaScript build scripts pass syntax checks. |
| Windows release | NSIS packaging passed. Installer: 42,850,594 bytes; SHA-256 `9df2da75aa8ba207d36bc6e9737c282d8954d414a69bbeed1e305658a4e4abfa`. `artifacts/unified-release-build.log`. |

The suites use synthetic libraries and protocol fixtures. No coursework was
sent to MinerU or Voyage and no billed model turn was requested. Current
Windows application identifiers and library paths are unchanged.

## Native builds and review

`.github/workflows/desktop.yml` builds and tests this branch independently on
Windows and macOS. Its downloadable artifacts are test builds; macOS uses
ad-hoc signing and Windows installers are unsigned. The workflow does not
publish a release or merge `andre` into `master`.

Before promoting the branch, check the native macOS app's sign-in, WebKit
navigation, Command shortcuts, file drops and agent chat on the user's Mac.
Compilation and unit tests cannot verify those interactive OS behaviours.
