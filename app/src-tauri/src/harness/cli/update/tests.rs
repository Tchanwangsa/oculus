use super::fetch::{arch, claude_channel, fetch, latest_endpoint, Body};
use super::version::{read_version, version_core};
use super::*;
use crate::harness::cli::discover;
use crate::harness::event::Provider;
use std::path::Path;

const HOME: &str = "/Users/s";

fn src(p: &str) -> Source {
    source_of(Path::new(p), Some(Path::new(HOME)))
}

#[test]
fn source_follows_the_real_path() {
    assert_eq!(
        src("/opt/homebrew/Caskroom/codex/0.153.4/codex-aarch64-apple-darwin"),
        Source::Brew
    );
    assert_eq!(
        src("/usr/local/Caskroom/claude-code/2.1.289/claude"),
        Source::Brew
    );
    assert_eq!(
        src("/opt/homebrew/Cellar/opencode/1.18.31/bin/opencode"),
        Source::Brew
    );
    // A formula that vendors a node package is still Homebrew's.
    assert_eq!(
        src("/opt/homebrew/Cellar/x/1/libexec/lib/node_modules/x/cli.js"),
        Source::Brew
    );
    // npm's global prefix under Homebrew's node is npm's.
    assert_eq!(
        src("/opt/homebrew/lib/node_modules/@openai/codex/bin/codex.js"),
        Source::Npm
    );
    assert_eq!(
        src("/Users/s/.nvm/versions/node/v22.1.0/lib/node_modules/opencode-ai/bin/opencode"),
        Source::Npm
    );
    assert_eq!(
        src("/Users/s/.bun/install/global/node_modules/opencode-ai/bin/opencode"),
        Source::Bun
    );
    assert_eq!(
        src("/Users/s/.local/share/claude/versions/2.1.289"),
        Source::SelfManaged
    );
    assert_eq!(src("/Users/s/.opencode/bin/opencode"), Source::SelfManaged);
    assert_eq!(src("/Users/s/.local/bin/agy"), Source::SelfManaged);
    assert_eq!(src("/opt/homebrew/bin/codex"), Source::Brew);
}

#[test]
fn versions_compare_numerically() {
    assert!(is_newer("1.18.34", "1.18.31"));
    assert!(is_newer("0.160.0", "0.153.4"));
    assert!(is_newer("v0.160.1", "0.153.4"));
    assert!(is_newer("1.10.0", "1.9.9"));
    assert!(is_newer("1.2.1", "1.2"));
    // Older or equal is never an update — a local build ahead included.
    assert!(!is_newer("2.1.285", "2.1.289"));
    assert!(!is_newer("2.1.289", "v2.1.289"));
    assert!(!is_newer("1.2", "1.2.0"));
    // Prerelease and build suffixes are ignored for ordering.
    assert!(!is_newer("1.2.0", "1.2.0-beta.1"));
    assert!(is_newer("1.3.0-rc.1", "1.2.9"));
    assert!(!is_newer("0.160.0,abc123", "0.160.0"));
}

#[test]
fn commands_match_the_install() {
    let p = Path::new("/Users/s/.local/bin/claude");
    assert_eq!(
        command(Provider::Claude, Source::SelfManaged, p),
        "'/Users/s/.local/bin/claude' update"
    );
    assert_eq!(
        command(Provider::Codex, Source::Brew, p),
        "brew upgrade --cask codex"
    );
    assert_eq!(
        command(Provider::Claude, Source::Brew, p),
        "brew upgrade --cask claude-code"
    );
    assert_eq!(
        command(Provider::Opencode, Source::Brew, p),
        "brew upgrade anomalyco/tap/opencode"
    );
    assert_eq!(
        command(Provider::Codex, Source::Npm, p),
        "npm install -g @openai/codex@latest"
    );
    assert_eq!(
        command(Provider::Opencode, Source::Bun, p),
        "bun install -g opencode-ai@latest"
    );
    let oc = Path::new("/Users/s/.opencode/bin/opencode");
    assert_eq!(
        command(Provider::Opencode, Source::SelfManaged, oc),
        "'/Users/s/.opencode/bin/opencode' upgrade"
    );
    let quoted = Path::new("/Users/it's/bin/agy");
    assert_eq!(
        command(Provider::Antigravity, Source::SelfManaged, quoted),
        r"'/Users/it'\''s/bin/agy' update"
    );
}

#[test]
fn unknown_pairings_fall_back_to_the_cli() {
    let agy = Path::new("/opt/homebrew/bin/agy");
    assert_eq!(
        command(Provider::Antigravity, Source::Brew, agy),
        "'/opt/homebrew/bin/agy' update"
    );
    assert_eq!(
        command(Provider::Antigravity, Source::Npm, agy),
        "'/opt/homebrew/bin/agy' update"
    );
    let claude = Path::new("/Users/s/.bun/bin/claude");
    assert_eq!(
        command(Provider::Claude, Source::Bun, claude),
        "'/Users/s/.bun/bin/claude' update"
    );
    // And they are compared against what that command installs.
    let (url, _) = latest_endpoint(Provider::Claude, Source::Bun, "latest", "arm64");
    assert!(url.starts_with("https://downloads.claude.ai/"), "{url}");
}

#[test]
fn no_command_needs_sudo() {
    let p = Path::new("/x/bin/cli");
    for provider in discover::PROVIDERS {
        for source in [Source::Brew, Source::Npm, Source::Bun, Source::SelfManaged] {
            let c = command(provider, source, p);
            assert!(!c.split_whitespace().any(|w| w == "sudo"), "{c}");
        }
    }
}

/// Homebrew's cask can trail npm, so a brew install is compared to brew.
#[test]
fn each_source_reads_its_own_registry() {
    let url = |p, s| latest_endpoint(p, s, "latest", "arm64").0;
    assert_eq!(
        url(Provider::Codex, Source::Brew),
        "https://formulae.brew.sh/api/cask/codex.json"
    );
    assert_eq!(
        url(Provider::Codex, Source::Npm),
        "https://registry.npmjs.org/@openai/codex/latest"
    );
    assert_eq!(
        url(Provider::Claude, Source::Brew),
        "https://formulae.brew.sh/api/cask/claude-code.json"
    );
    assert_eq!(
        url(Provider::Claude, Source::Npm),
        "https://registry.npmjs.org/@anthropic-ai/claude-code/latest"
    );
    assert_eq!(
        url(Provider::Claude, Source::SelfManaged),
        "https://downloads.claude.ai/claude-code-releases/latest"
    );
    assert_eq!(
        latest_endpoint(Provider::Claude, Source::SelfManaged, "stable", "arm64").0,
        "https://downloads.claude.ai/claude-code-releases/stable"
    );
    assert_eq!(
        url(Provider::Opencode, Source::Brew),
        "https://registry.npmjs.org/opencode-ai/latest"
    );
    assert!(
        url(Provider::Antigravity, Source::SelfManaged).ends_with("/manifests/darwin_arm64.json")
    );
}

#[test]
fn bodies_yield_a_version_or_an_error() {
    assert_eq!(read_version("2.1.289\n", Body::Text).unwrap(), "2.1.289");
    assert_eq!(
        read_version(r#"{"name":"x","version":"1.18.34"}"#, Body::JsonVersion).unwrap(),
        "1.18.34"
    );
    assert!(read_version("<html>", Body::Text).is_err());
    assert!(read_version(r#"{"error":"not found"}"#, Body::JsonVersion).is_err());
}

#[test]
fn claude_channel_reads_its_settings() {
    let dir = crate::test_support::Scratch::new("update-channel");
    assert_eq!(claude_channel(Some(&*dir)), "latest");
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    std::fs::write(
        dir.join(".claude/settings.json"),
        r#"{"autoUpdatesChannel":"stable"}"#,
    )
    .unwrap();
    assert_eq!(claude_channel(Some(&*dir)), "stable");
}

/// Hits the real endpoints: `cargo test --lib harness::cli::update -- --ignored`.
#[test]
#[ignore]
fn live_endpoints_answer() {
    for (p, s) in [
        (Provider::Claude, Source::SelfManaged),
        (Provider::Claude, Source::Brew),
        (Provider::Claude, Source::Npm),
        (Provider::Codex, Source::Brew),
        (Provider::Codex, Source::Npm),
        (Provider::Opencode, Source::SelfManaged),
        (Provider::Antigravity, Source::SelfManaged),
    ] {
        let (url, shape) = latest_endpoint(p, s, "latest", arch());
        let v = fetch(&url, shape).unwrap_or_else(|e| panic!("{p:?} {s:?}: {e}"));
        assert!(!version_core(&v).is_empty(), "{url}: {v}");
    }
}
