//! The sessions in the vault. Every change bumps the in-memory `Generation`
//! after the vault write has landed.

use super::cookie::merge_set_cookie;
use super::{Generation, Kind, MAX_VALUE};
use crate::vault::{Entries, Vault, VaultError};

/// The session, or `None` when it is absent or empty.
pub fn get(vault: &Vault, kind: Kind) -> Result<Option<String>, VaultError> {
    Ok(vault.get(kind.secret())?.filter(|v| !v.is_empty()))
}

/// Which of the three are held, from one read.
pub fn held(vault: &Vault) -> Result<Vec<Kind>, VaultError> {
    let entries = vault.load()?;
    Ok(Kind::ALL
        .into_iter()
        .filter(|k| entries.get(k.secret()).is_some_and(|v| !v.is_empty()))
        .collect())
}

/// Replaces the whole session.
pub fn put(
    vault: &Vault,
    generation: &Generation,
    kind: Kind,
    value: &str,
) -> Result<(), VaultError> {
    vault.update(|e| e.insert(kind.secret(), value))?;
    generation.bump();
    Ok(())
}

/// `put`, when `allowed` passes on the entries it would write into, under the
/// same lock: a check of the site cannot be overtaken by a `site_set`.
pub fn put_if<E>(
    vault: &Vault,
    generation: &Generation,
    kind: Kind,
    value: &str,
    allowed: impl FnOnce(&Entries) -> Result<(), E>,
) -> Result<Result<(), E>, VaultError> {
    let put = vault.update(|e| {
        allowed(e)?;
        e.insert(kind.secret(), value);
        Ok(())
    })?;
    if put.is_ok() {
        generation.bump();
    }
    Ok(put)
}

/// Removes `kinds`; the ones that were held come back.
pub fn clear(
    vault: &Vault,
    generation: &Generation,
    kinds: &[Kind],
) -> Result<Vec<Kind>, VaultError> {
    clear_with(vault, generation, kinds, |_| ())
}

/// `clear`, with `also` applied in the same write. With no `kinds` the
/// generation stays: no session changed.
pub fn clear_with(
    vault: &Vault,
    generation: &Generation,
    kinds: &[Kind],
    also: impl FnOnce(&mut Entries),
) -> Result<Vec<Kind>, VaultError> {
    let removed = vault.update(|e| {
        also(e);
        kinds
            .iter()
            .copied()
            .filter(|k| e.remove(k.secret()))
            .collect::<Vec<_>>()
    })?;
    if !kinds.is_empty() {
        generation.bump();
    }
    Ok(removed)
}

/// Folds an origin's `Set-Cookie` lines into the held cookie header, under
/// the vault's lock so two answers cannot overwrite each other. True when the
/// session changed. A cleared session stays cleared, a result too long for
/// the wire is not kept, and nothing is merged unless `still` holds for the
/// entries as they are now: an answer from a site keyd has since left must
/// not land in the next site's session.
pub fn absorb(
    vault: &Vault,
    generation: &Generation,
    kind: Kind,
    set_cookies: &[String],
    still: impl FnOnce(&Entries) -> bool,
) -> Result<bool, VaultError> {
    let changed = vault.update(|e| {
        if !still(e) {
            return false;
        }
        let held = e.get(kind.secret()).unwrap_or("");
        // A rotation never makes a session: one cleared since the request
        // was sent (a sign-out) stays cleared.
        if held.is_empty() {
            return false;
        }
        let Some(merged) = merge_set_cookie(held, set_cookies) else {
            return false;
        };
        if merged.len() > MAX_VALUE {
            return false;
        }
        if merged.is_empty() {
            e.remove(kind.secret());
        } else {
            e.insert(kind.secret(), &merged);
        }
        true
    })?;
    if changed {
        generation.bump();
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use crate::vault::MasterKey;

    fn vault(dir: &Scratch) -> Vault {
        Vault::new(crate::paths::vault(&dir.0), MasterKey::from_bytes([7; 32]))
    }

    fn lines(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn put_get_clear_and_held() {
        let dir = Scratch::new("session-store");
        let (vault, generation) = (vault(&dir), Generation::default());
        assert_eq!(get(&vault, Kind::Canvas).unwrap(), None);
        put(&vault, &generation, Kind::Canvas, "a=1").unwrap();
        put(&vault, &generation, Kind::Ed, "jwt").unwrap();
        assert_eq!(get(&vault, Kind::Canvas).unwrap().as_deref(), Some("a=1"));
        assert_eq!(held(&vault).unwrap(), [Kind::Canvas, Kind::Ed]);
        assert_eq!(generation.get(), 2);

        let removed = clear(&vault, &generation, &[Kind::Canvas, Kind::Sso]).unwrap();
        assert_eq!(removed, [Kind::Canvas]);
        assert_eq!(held(&vault).unwrap(), [Kind::Ed]);
        assert_eq!(generation.get(), 3);
    }

    #[test]
    fn absorb_does_not_bring_back_a_session_that_was_cleared() {
        let dir = Scratch::new("session-absorb-cleared");
        let (vault, generation) = (vault(&dir), Generation::default());
        put(&vault, &generation, Kind::Canvas, "canvas_session=OLD").unwrap();
        clear(&vault, &generation, &[Kind::Canvas]).unwrap();
        let set = lines(&["canvas_session=ROTATED; path=/; httponly"]);
        assert!(!absorb(&vault, &generation, Kind::Canvas, &set, |_| true).unwrap());
        assert_eq!(get(&vault, Kind::Canvas).unwrap(), None);
        assert_eq!(generation.get(), 2, "nothing changed");
    }

    #[test]
    fn absorb_merges_removes_and_counts_only_real_changes() {
        let dir = Scratch::new("session-absorb");
        let (vault, generation) = (vault(&dir), Generation::default());
        put(&vault, &generation, Kind::Canvas, "a=1; canvas_session=OLD").unwrap();

        let set = lines(&["canvas_session=NEW; path=/; httponly", "b=2"]);
        assert!(absorb(&vault, &generation, Kind::Canvas, &set, |_| true).unwrap());
        assert_eq!(
            get(&vault, Kind::Canvas).unwrap().as_deref(),
            Some("a=1; canvas_session=NEW; b=2")
        );
        assert_eq!(generation.get(), 2);

        assert!(!absorb(&vault, &generation, Kind::Canvas, &set, |_| true).unwrap());
        assert_eq!(generation.get(), 2, "an unchanged cookie is no change");

        let gone = lines(&["canvas_session=; Max-Age=0"]);
        assert!(absorb(&vault, &generation, Kind::Canvas, &gone, |_| true).unwrap());
        assert_eq!(
            get(&vault, Kind::Canvas).unwrap().as_deref(),
            Some("a=1; b=2")
        );
    }

    #[test]
    fn absorb_never_makes_a_session_and_empties_one_cleanly() {
        let dir = Scratch::new("session-absorb-none");
        let (vault, generation) = (vault(&dir), Generation::default());
        assert!(
            !absorb(&vault, &generation, Kind::Canvas, &lines(&["a=1"]), |_| {
                true
            })
            .unwrap()
        );
        assert_eq!(get(&vault, Kind::Canvas).unwrap(), None);
        assert_eq!(generation.get(), 0);

        put(&vault, &generation, Kind::Canvas, "a=1").unwrap();
        assert!(absorb(
            &vault,
            &generation,
            Kind::Canvas,
            &lines(&["a=; Max-Age=0"]),
            |_| true
        )
        .unwrap());
        assert!(!vault.has(Kind::Canvas.secret()).unwrap(), "no empty entry");
    }

    #[test]
    fn absorb_merges_nothing_once_its_guard_fails() {
        let dir = Scratch::new("session-absorb-guard");
        let (vault, generation) = (vault(&dir), Generation::default());
        put(&vault, &generation, Kind::Canvas, "canvas_session=B").unwrap();
        let set = lines(&["canvas_session=FROM-A; path=/"]);
        assert!(!absorb(&vault, &generation, Kind::Canvas, &set, |_| false).unwrap());
        assert_eq!(
            get(&vault, Kind::Canvas).unwrap().as_deref(),
            Some("canvas_session=B")
        );
        assert_eq!(generation.get(), 1);
    }

    #[test]
    fn put_if_writes_nothing_when_refused() {
        let dir = Scratch::new("session-put-if");
        let (vault, generation) = (vault(&dir), Generation::default());
        let refused = put_if(&vault, &generation, Kind::Canvas, "a=1", |_| Err("no")).unwrap();
        assert_eq!(refused, Err("no"));
        assert_eq!(get(&vault, Kind::Canvas).unwrap(), None);
        assert_eq!(generation.get(), 0);
        put_if(&vault, &generation, Kind::Canvas, "a=1", |_| {
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap();
        assert_eq!(get(&vault, Kind::Canvas).unwrap().as_deref(), Some("a=1"));
        assert_eq!(generation.get(), 1);
    }

    #[test]
    fn a_merge_over_the_cap_is_not_kept() {
        let dir = Scratch::new("session-absorb-big");
        let (vault, generation) = (vault(&dir), Generation::default());
        put(&vault, &generation, Kind::Canvas, "a=1").unwrap();
        let big = lines(&[&format!("b={}", "x".repeat(MAX_VALUE))]);
        assert!(!absorb(&vault, &generation, Kind::Canvas, &big, |_| true).unwrap());
        assert_eq!(get(&vault, Kind::Canvas).unwrap().as_deref(), Some("a=1"));
    }
}
