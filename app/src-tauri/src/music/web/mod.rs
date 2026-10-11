//! Spotify's Web API, for what the Connect protocol cannot do: list the
//! account's playlists and search. The token is Login5's for the live session
//! (`token.rs`), so there is no Client ID of our own. That client id is
//! throttled hard, so calls are few: the playlists are fetched once per
//! account and kept until asked again, search runs only when asked, a 429
//! closes a gate until its `Retry-After` (`gate.rs`), and nothing is retried
//! but a 401, once, with a new token.
//!
//! See `docs/music.md`.

mod error;
mod gate;
mod http;
mod images;
mod playlists;
mod search;
#[cfg(test)]
mod tests;
mod token;

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tauri::AppHandle;

pub(super) use error::WebError;
pub use playlists::Playlist;
pub use search::SearchResults;

use super::state;
use gate::Gate;
use http::API;
use token::Cached;

#[derive(Default)]
pub struct Web(Mutex<Inner>);

#[derive(Default)]
struct Inner {
    token: Option<Cached>,
    gate: Gate,
    /// The account they belong to, and its playlists.
    playlists: Option<(String, Vec<Playlist>)>,
}

impl Web {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn token(&self) -> Option<Cached> {
        self.lock().token.clone()
    }

    fn set_token(&self, token: Option<Cached>) {
        self.lock().token = token;
    }

    fn check_gate(&self) -> Result<(), WebError> {
        match self.lock().gate.closed_for(Instant::now()) {
            Some(secs) => Err(WebError::RateLimited(secs)),
            None => Ok(()),
        }
    }

    fn close_gate(&self, wait: Duration) {
        self.lock().gate.close(Instant::now(), wait);
    }

    fn playlists_for(&self, account: &str) -> Option<Vec<Playlist>> {
        match &self.lock().playlists {
            Some((owner, list)) if owner == account => Some(list.clone()),
            _ => None,
        }
    }

    fn set_playlists(&self, account: String, list: Vec<Playlist>) {
        self.lock().playlists = Some((account, list));
    }
}

/// The account's playlists, from memory unless `refresh` or the account
/// changed. Up to `playlists::CAP`, fifty to a call.
pub(super) async fn playlists(app: &AppHandle, refresh: bool) -> Result<Vec<Playlist>, WebError> {
    let (session, _) = state::session(app).ok_or(WebError::NotConnected)?;
    let account = session.username();
    let web = state::web(app);
    if !refresh {
        if let Some(list) = web.playlists_for(&account) {
            return Ok(list);
        }
    }
    let mut url = format!("{API}me/playlists?limit={}", playlists::PAGE);
    let mut all = Vec::new();
    loop {
        let page = playlists::parse_page(&call(app, url).await?)?;
        all.extend(page.items);
        match page.next {
            Some(next) if all.len() < playlists::CAP => url = next,
            _ => break,
        }
    }
    all.truncate(playlists::CAP);
    web.set_playlists(account, all.clone());
    Ok(all)
}

/// Playlists, albums and tracks for `query`; an empty query asks nothing.
pub(super) async fn search(app: &AppHandle, query: &str) -> Result<SearchResults, WebError> {
    let Some(query) = search::clean(query) else {
        return Ok(SearchResults::default());
    };
    if state::session(app).is_none() {
        return Err(WebError::NotConnected);
    }
    search::parse(&call(app, search::url(&query)).await?)
}

/// One GET through the gate, with one new token after a 401.
async fn call(app: &AppHandle, url: String) -> Result<String, WebError> {
    let web = state::web(app);
    web.check_gate()?;
    let token = token::get(app, None).await?;
    let result = match http::get(token.clone(), url.clone()).await {
        Err(WebError::Unauthorized) => {
            let fresh = token::get(app, Some(&token)).await?;
            http::get(fresh, url).await.map_err(|e| match e {
                WebError::Unauthorized => WebError::StillUnauthorized,
                e => e,
            })
        }
        result => result,
    };
    if let Err(WebError::RateLimited(secs)) = &result {
        web.close_gate(Duration::from_secs(*secs));
    }
    if let Err(e) = &result {
        eprintln!("[music] Web API: {e}");
    }
    result
}
