use super::flow::login_args;
use super::output::{extract_url, strip_ansi};
use super::status::{codex_account, parse_claude_status};
use super::*;
use crate::harness::event::Provider;

#[test]
fn the_real_expired_session_message_is_an_auth_failure() {
    let msg = "Failed to authenticate: OAuth session expired and could not be refreshed";
    assert!(is_auth_failure(Provider::Claude, msg));
}

#[test]
fn each_provider_knows_its_own_wording() {
    assert!(is_auth_failure(
        Provider::Claude,
        "Please run /login to authenticate"
    ));
    assert!(is_auth_failure(
        Provider::Claude,
        "Credentials are invalid, run `claude auth login`"
    ));
    assert!(is_auth_failure(
        Provider::Codex,
        "Not logged in. Run `codex login`."
    ));
    assert!(is_auth_failure(
        Provider::Codex,
        "Your ChatGPT account subscription has expired"
    ));
    assert!(is_auth_failure(
        Provider::Opencode,
        "AI_APICallError: incorrect API key provided"
    ));
    assert!(is_auth_failure(
        Provider::Opencode,
        "no credentials for provider anthropic"
    ));
    for p in [Provider::Claude, Provider::Codex, Provider::Opencode] {
        assert!(is_auth_failure(p, "OAuth token has expired"), "{p:?}");
        assert!(
            is_auth_failure(p, "authentication_error: invalid x-api-key"),
            "{p:?}"
        );
    }
}

#[test]
fn ordinary_failures_are_not_auth_failures() {
    let innocent = [
        "error[E0308]: mismatched types",
        "No such file or directory (os error 2)",
        "rate limit reached, retry after 30s",
        "wrote 401 bytes to /tmp/oculus/out.log",
        "/var/log/401/report.txt: not found",
        "claude exited (code Some(1)) mid-turn",
        "turn failed: the model returned no content",
        "codex app-server exited (code Some(2))",
        "ENOENT: could not open agents/notes.md",
    ];
    for p in [Provider::Claude, Provider::Codex, Provider::Opencode] {
        for msg in innocent {
            assert!(!is_auth_failure(p, msg), "{p:?} wrongly flagged {msg:?}");
        }
    }
}

#[test]
fn a_bare_401_is_not_enough() {
    assert!(!is_auth_failure(Provider::Claude, "server answered 401"));
    assert!(is_auth_failure(Provider::Claude, "401 Unauthorized"));
    assert!(is_auth_failure(
        Provider::Codex,
        "HTTP 401 while refreshing auth token"
    ));
}

/// Claude's block, verbatim; the prompt carries no trailing newline.
#[test]
fn claude_login_output_yields_its_authorize_url() {
    let block = "Opening browser to sign in…\n\
         If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&client_id=abc123\n\
         Paste code here if prompted >";
    let urls: Vec<String> = block.lines().filter_map(extract_url).collect();
    assert_eq!(
        urls,
        vec!["https://claude.com/cai/oauth/authorize?code=true&client_id=abc123"]
    );
}

/// Codex's block, verbatim.
#[test]
fn codex_login_output_skips_the_loopback_line() {
    let block = "Starting local login server on http://localhost:1455.\n\
         If your browser did not open, navigate to this URL to authenticate:\n\
         \n\
         https://auth.openai.com/oauth/authorize?client_id=app_X&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid\n\
         \n\
         On a remote or headless machine? Use `codex login --device-auth` instead.";
    let urls: Vec<String> = block.lines().filter_map(extract_url).collect();
    assert_eq!(
        urls,
        vec!["https://auth.openai.com/oauth/authorize?client_id=app_X&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid"]
    );
}

#[test]
fn a_url_is_trimmed_of_the_sentence_around_it() {
    assert_eq!(
        extract_url("visit https://example.com/a?b=1.").as_deref(),
        Some("https://example.com/a?b=1")
    );
    assert_eq!(
        extract_url("(see https://example.com/a)").as_deref(),
        Some("https://example.com/a")
    );
    assert_eq!(extract_url("nothing to open here"), None);
    assert_eq!(extract_url("http://localhost:1455/auth/callback"), None);
}

#[test]
fn ansi_is_stripped_before_the_url_is_read() {
    let line =
        "\u{1b}[1mvisit:\u{1b}[0m \u{1b}[4mhttps://auth.openai.com/oauth/authorize?x=1\u{1b}[0m";
    let clean = strip_ansi(line);
    assert_eq!(clean, "visit: https://auth.openai.com/oauth/authorize?x=1");
    assert_eq!(
        extract_url(&clean).as_deref(),
        Some("https://auth.openai.com/oauth/authorize?x=1")
    );
}

#[test]
fn claude_status_json_parses_both_answers() {
    let out = r#"{"loggedIn": false, "authMethod": "none", "apiProvider": "firstParty"}"#;
    assert_eq!(parse_claude_status(out), Some((false, None)));

    let out = r#"{"loggedIn": true, "authMethod": "claude.ai", "apiProvider": "firstParty",
                  "email": "someone@example.com", "subscriptionType": "max"}"#;
    assert_eq!(
        parse_claude_status(out),
        Some((true, Some("someone@example.com".to_string())))
    );

    let out = r#"{"loggedIn": true, "authMethod": "claude.ai"}"#;
    assert_eq!(
        parse_claude_status(out),
        Some((true, Some("Claude subscription".to_string())))
    );

    let out = r#"{"loggedIn":true,"authMethod":"console"}"#;
    assert_eq!(
        parse_claude_status(out),
        Some((true, Some("Anthropic Console".to_string())))
    );

    // Unreadable is "cannot say", not "signed out".
    assert_eq!(parse_claude_status("Unknown command: auth"), None);
    assert_eq!(parse_claude_status(r#"{"authMethod":"claude.ai"}"#), None);
}

#[test]
fn codex_status_line_names_the_account() {
    assert_eq!(
        codex_account("Logged in using ChatGPT\n").as_deref(),
        Some("ChatGPT")
    );
    assert_eq!(
        codex_account("Logged in using an API key\n").as_deref(),
        Some("an API key")
    );
    assert_eq!(codex_account("\n\n").as_deref(), None);
}

#[test]
fn opencode_is_routed_to_its_own_dialog() {
    let s = status(Provider::Opencode);
    assert_eq!(s.signed_in, None);
    assert_eq!(s.error, None);
    assert!(start(Provider::Opencode, |_| {}).is_err());
    assert!(login_args(Provider::Opencode).is_err());
}

#[test]
fn code_and_cancel_need_a_run() {
    assert!(submit_code(Provider::Claude, "abc").is_err());
    assert!(cancel(Provider::Codex).is_err());
}
