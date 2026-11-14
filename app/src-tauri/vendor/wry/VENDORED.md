# Vendored wry

wry 0.55.1 as published on crates.io (upstream commit
`a5bf203a1c8dbb3583588382538d6521655222a8`), patched in through
`[patch.crates-io]` in `app/src-tauri/Cargo.toml`.

Left out of the copy: `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml.orig`,
`MOBILE.md`, `SECURITY.md`, `renovate.json`, `.cargo/`, `.gitignore` and
`.license_template`.

## Changes from upstream

1. **HTTP attachments download on WebKit (upstream: yes).** Oculus needs
   attachment responses, including PDFs and responses in subframes, to reach
   its download handler instead of displaying in the tab. In
   `src/wkwebview/navigation.rs`, `navigation_policy_response` requests
   `WKNavigationResponsePolicy::Download` when a started handler exists and
   either WebKit cannot show the MIME type or `is_attachment` returns true;
   that helper downcasts to `NSHTTPURLResponse`, reads `Content-Disposition`,
   and compares the trimmed text before the first semicolon to `attachment`
   without case sensitivity. `Cargo.toml` enables the `NSURLResponse` feature
   on `objc2-foundation` explicitly.

2. **WebKit context-menu downloads use the download delegate (upstream: no,
   private API).** Oculus needs WebKit's Download Image, Download Linked File
   and Download Video actions to reach its ordinary handlers with WebKit's
   request, cookies and filename. In
   `src/wkwebview/class/wry_navigation_delegate.rs`,
   `WryNavigationDelegate::context_menu_did_create_download` implements the
   private macOS 12+ selector `_webView:contextMenuDidCreateDownload:` and
   forwards to `context_menu_download` in `src/wkwebview/download.rs`; that
   function attaches the navigation delegate's `download_delegate` to the
   `WKDownload` if present and does nothing otherwise.

3. **WebKit completion carries the accepted destination (upstream: yes).**
   Oculus matches completed downloads by path because simultaneous downloads
   can share a URL and finish in either order. In
   `src/wkwebview/class/wry_download_delegate.rs`,
   `WryDownloadDelegateIvars::destinations` is a
   `RefCell<HashMap<usize, PathBuf>>`, initialised empty by
   `WryDownloadDelegate::new`; in `src/wkwebview/download.rs`, `download_key`
   uses the `WKDownload` address as its key, `download_policy` records the
   final destination only after acceptance, and `download_did_finish` and
   `download_did_fail` remove and pass that path to the completed handler.
   A download with no accepted destination reports `None`.

4. **The started handler sees WebKit's suggested filename (upstream:
   maybe).** Oculus needs the server's filename without collision numbering
   from the user's Downloads folder when it chooses its own staging path.
   In `src/wkwebview/download.rs`, `download_policy` passes the unnumbered
   Downloads-directory destination to the started handler, then checks for
   existing files and appends numbers only if the accepted path still equals
   that default destination; redirected destinations skip collision checks
   there. The existing numbering splits at the first dot and inserts
   ` (1)`, ` (2)`, etc. before the remaining extension.

5. **No started handler by default (upstream: no, local policy).** Oculus
   opts browser tabs into downloads explicitly and needs its other macOS
   webviews to avoid silent writes into the user's Downloads folder. In
   `src/lib.rs`, `WebViewAttributes::default` sets
   `download_started_handler: None`; the field and
   `WebViewBuilder::with_download_started_handler` document that default.
   With both download handlers unset, WebKit's response policy allows
   navigation, its `<a download>` action policy cancels, and context-menu
   downloads receive no download delegate; Windows and Linux register no
   wry download callbacks and retain their engine's default behaviour.

## Updating

The version here must equal the one `tauri-runtime-wry` resolves, or cargo
drops the patch without an error. After a tauri bump, copy the new wry
release over this folder, reapply all five changes listed above, and check that
`cargo tree -i wry` prints this path.
