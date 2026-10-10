//! The sessions in the vault. Every change bumps the in-memory `Generation`
//! after the vault write has landed.

use super::cookie::merge_set_cookie;
use super::{Generation, Kind, MAX_VALUE};
use crate::vault::{Vault, VaultError};

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

/// Removes `kinds`; the ones that were held come back.
pub fn clear(
    vault: &Vault,
    generation: &Generation,
    kinds: &[Kind],
) -> Result<Vec<Kind>, VaultError> {
    let removed = vault.update(|e| {
        kinds
            .iter()
            .copied()
            .filter(|k| e.remove(k.secret()))
            .collect::<Vec<_>>()
    })?;
    generation.bump();
    Ok(removed)
}

/// Folds an origin's `Set-Cookie` lines into the held cookie header, under
/// the vault's lock so two answers cannot overwrite each other. True when the
/// session changed. A cleared session stays cleared, and a result too long
/// for the wire is not kept.
pub fn absorb(
    vault: &Vault,
    generation: &Generation,
    kind: Kind,
    set_cookies: &[String],
) -> Result<bool, VaultError> {
    let changed = vault.update(|e| {
        let Some(merged) = merge_set_cookie(e.get(kind.secret()).unwrap_or(""), set_cookies) else {
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
    fn absorb_merges_removes_and_counts_only_real_changes() {
        let dir = Scratch::new("session-absorb");
        let (vault, generation) = (vault(&dir), Generation::default());
        put(&vault, &generation, Kind::Canvas, "a=1; canvas_session=OLD").unwrap();

        let set = lines(&["canvas_session=NEW; path=/; httponly", "b=2"]);
        assert!(absorb(&vault, &generation, Kind::Canvas, &set).unwrap());
        assert_eq!(
            get(&vault, Kind::Canvas).unwrap().as_deref(),
            Some("a=1; canvas_session=NEW; b=2")
        );
        assert_eq!(generation.get(), 2);

        assert!(!absorb(&vault, &generation, Kind::Canvas, &set).unwrap());
        assert_eq!(generation.get(), 2, "an unchanged cookie is no change");

        let gone = lines(&["canvas_session=; Max-Age=0"]);
        assert!(absorb(&vault, &generation, Kind::Canvas, &gone).unwrap());
        assert_eq!(
            get(&vault, Kind::Canvas).unwrap().as_deref(),
            Some("a=1; b=2")
        );
    }

    #[test]
    fn absorb_never_makes_a_session_and_empties_one_cleanly() {
        let dir = Scratch::new("session-absorb-none");
        let (vault, generation) = (vault(&dir), Generation::default());
        assert!(!absorb(&vault, &generation, Kind::Canvas, &lines(&["a=1"])).unwrap());
        assert_eq!(get(&vault, Kind::Canvas).unwrap(), None);
        assert_eq!(generation.get(), 0);

        put(&vault, &generation, Kind::Canvas, "a=1").unwrap();
        assert!(absorb(
            &vault,
            &generation,
            Kind::Canvas,
            &lines(&["a=; Max-Age=0"])
        )
        .unwrap());
        assert!(!vault.has(Kind::Canvas.secret()).unwrap(), "no empty entry");
    }

    #[test]
    fn a_merge_over_the_cap_is_not_kept() {
        let dir = Scratch::new("session-absorb-big");
        let (vault, generation) = (vault(&dir), Generation::default());
        put(&vault, &generation, Kind::Canvas, "a=1").unwrap();
        let big = lines(&[&format!("b={}", "x".repeat(MAX_VALUE))]);
        assert!(!absorb(&vault, &generation, Kind::Canvas, &big).unwrap());
        assert_eq!(get(&vault, Kind::Canvas).unwrap().as_deref(), Some("a=1"));
    }
}
