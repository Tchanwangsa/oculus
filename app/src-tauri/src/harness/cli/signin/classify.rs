//! Whether a provider's error text means "sign in again".

use crate::harness::event::Provider;

/// Phrases that mean "no usable credentials" whichever CLI said them. Whole
/// clauses, never words: `"auth"` alone would match `auth.rs` in a compile
/// error.
const SHARED: &[&str] = &[
    "oauth token has expired",
    "oauth session expired",
    "session expired",
    "not logged in",
    "not authenticated",
    "invalid api key",
    "authentication_error",
    "authentication failed",
    "invalid_grant",
];

const CLAUDE: &[&str] = &[
    "failed to authenticate",
    "please run /login",
    "run /login",
    "claude auth login",
    "credentials are invalid",
];

/// `"chatgpt account"` appears in happy status output too, so for Codex it
/// only counts beside one of [`CODEX_ACCOUNT_TROUBLE`].
const CODEX: &[&str] = &["codex login", "please sign in"];
const CODEX_ACCOUNT_TROUBLE: &[&str] = &[
    "expired", "not ", "no ", "missing", "invalid", "failed", "required", "sign in",
];

/// What an opencode provider answers when its key is wrong or absent.
const OPENCODE: &[&str] = &[
    "opencode auth login",
    "no credentials",
    "provider is not connected",
    "incorrect api key",
    "invalid x-api-key",
];

/// Antigravity signs in through Google and keeps the credential in the keyring.
const ANTIGRAVITY: &[&str] = &[
    "please sign in",
    "google sign-in",
    "sign in to antigravity",
    "not signed in",
    "no credentials found in keyring",
    "reauthenticate",
];

/// Whether `msg` is the provider saying it has no usable credentials — which
/// puts a Sign in button on the row, so the lists stay tight. `401` counts
/// only beside "auth": a byte count or a path can carry those digits.
pub fn is_auth_failure(provider: Provider, msg: &str) -> bool {
    let m = msg.to_lowercase();
    if SHARED.iter().any(|n| m.contains(n)) {
        return true;
    }
    let own = match provider {
        Provider::Claude => CLAUDE,
        Provider::Codex => CODEX,
        Provider::Opencode => OPENCODE,
        Provider::Antigravity => ANTIGRAVITY,
    };
    if own.iter().any(|n| m.contains(n)) {
        return true;
    }
    if provider == Provider::Codex
        && m.contains("chatgpt account")
        && CODEX_ACCOUNT_TROUBLE.iter().any(|w| m.contains(w))
    {
        return true;
    }
    m.contains("401") && (m.contains("unauth") || m.contains("auth"))
}
