//! Deleting a key also deletes the keychain item it was imported from, so an
//! uninstalled keyd cannot leave the app's keychain fallback reading a key the
//! person deleted. Only deletes do this: after an import the old item stays,
//! because the app still reads it when keyd is absent.

use std::sync::PoisonError;

use super::State;
use crate::names;
use crate::vault::KeyError;

/// What happened to the old keychain items behind a deleted key. The ops
/// reply it as `"legacy"` and log it as `legacy=…`; it carries no value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Removal {
    /// No old item was there.
    Absent,
    /// At least one old item was deleted.
    Removed,
    /// The keychain failed on one (not a refusal); the rest were still tried.
    Failed,
    /// The keychain refused: a cancelled prompt or no access. Nothing after
    /// the refused item was tried, so a person who said no is not asked again.
    Refused,
}

impl Removal {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Removal::Absent => "absent",
            Removal::Removed => "removed",
            Removal::Failed => "failed",
            Removal::Refused => "refused",
        }
    }
}

impl State {
    /// Deletes the old items behind `secrets`, best effort: the vault's delete
    /// has already happened and is reported whatever this finds. Run after the
    /// vault update, which set the imported markers, so an import racing this
    /// cannot bring a deleted key back.
    pub(super) fn remove_legacy(&self, secrets: &[&str]) -> Removal {
        let _one = self
            .importing
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut worst = Removal::Absent;
        for item in names::LEGACY.iter().filter(|l| secrets.contains(&l.secret)) {
            let found = match self.legacy.remove(item.service, item.account) {
                Ok(true) => Removal::Removed,
                Ok(false) => Removal::Absent,
                Err(KeyError::Refused(_)) => return Removal::Refused,
                Err(KeyError::Platform(_)) => Removal::Failed,
            };
            // Failed outranks Removed so a half-deleted set is never reported done.
            worst = worst.max(found);
        }
        worst
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::testing::{call, cli, key, op_as};
    use super::super::State;
    use crate::names;
    use crate::test_support::{OldItems, Reads, Removed, Scratch, BUILD};
    use crate::vault::{KeyError, StaticKey, Vault};

    const SSO: &str = "com.oculus.unimelb-sso";
    const VOYAGE: (&str, &str) = ("com.tchan.oculus.voyage", "voyage");

    fn state_over(dir: &Scratch, items: OldItems) -> (State, Removed) {
        let removed = items.removed();
        let state = State::new(
            BUILD,
            dir.0.clone(),
            Box::new(StaticKey(key())),
            Box::new(items),
        );
        (state, removed)
    }

    fn items(held: Vec<((&'static str, &'static str), &str)>) -> OldItems {
        OldItems::new(
            held.into_iter()
                .map(|(k, v)| (k, Ok(Some(v.to_string()))))
                .collect(),
            Reads::default(),
        )
    }

    fn vault_of(dir: &Scratch) -> Vault {
        Vault::new(crate::paths::vault(&dir.0), key())
    }

    #[test]
    fn deleting_a_key_removes_the_vault_entry_and_the_old_item() {
        let dir = Scratch::new("legacy-delete");
        let (state, removed) = state_over(&dir, items(vec![(VOYAGE, "pa-old")]));
        call(
            &state,
            "store",
            json!({"secret": "voyage", "value": "pa-new"}),
        )
        .unwrap();
        let reply = call(&state, "delete", json!({"secret": "voyage"})).unwrap();
        assert_eq!(reply, json!({"existed": true, "legacy": "removed"}));
        assert_eq!(vault_of(&dir).get("voyage").unwrap(), None);
        assert_eq!(
            *removed.lock().unwrap(),
            vec![(VOYAGE.0.to_string(), VOYAGE.1.to_string())]
        );
    }

    #[test]
    fn the_other_cloud_keys_remove_their_own_items_only() {
        let dir = Scratch::new("legacy-delete-others");
        let mineru = ("com.tchan.oculus.mineru", "mineru");
        let groq = ("com.tchan.oculus.groq", "groq");
        let (state, removed) =
            state_over(&dir, items(vec![(VOYAGE, "v"), (mineru, "m"), (groq, "g")]));
        for name in ["mineru", "groq"] {
            let reply = call(&state, "delete", json!({"secret": name})).unwrap();
            assert_eq!(reply["legacy"], "removed", "{name}");
        }
        let gone = removed.lock().unwrap().clone();
        assert_eq!(gone.len(), 2);
        assert!(!gone.contains(&(VOYAGE.0.to_string(), VOYAGE.1.to_string())));
    }

    #[test]
    fn no_old_item_is_fine_and_the_second_delete_finds_none() {
        let dir = Scratch::new("legacy-absent");
        let (state, removed) = state_over(&dir, items(vec![]));
        let reply = call(&state, "delete", json!({"secret": "voyage"})).unwrap();
        assert_eq!(reply, json!({"existed": false, "legacy": "absent"}));

        let dir = Scratch::new("legacy-twice");
        let (state, removed2) = state_over(&dir, items(vec![(VOYAGE, "pa-old")]));
        assert_eq!(
            call(&state, "delete", json!({"secret": "voyage"})).unwrap()["legacy"],
            "removed"
        );
        assert_eq!(
            call(&state, "delete", json!({"secret": "voyage"})).unwrap()["legacy"],
            "absent"
        );
        assert!(removed.lock().unwrap().is_empty());
        assert_eq!(removed2.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_refused_removal_is_reported_and_the_vault_delete_stands_and_is_never_reimported() {
        let dir = Scratch::new("legacy-refused");
        let refused = KeyError::Refused("deleting x: OSStatus -128".into());
        let (state, removed) = state_over(
            &dir,
            items(vec![(VOYAGE, "pa-old")]).failing_removal(refused.clone()),
        );
        call(
            &state,
            "store",
            json!({"secret": "voyage", "value": "pa-new"}),
        )
        .unwrap();
        let reply = state
            .dispatch(&cli(), "delete", &json!({"secret": "voyage"}), b"")
            .unwrap();
        assert_eq!(reply.header, json!({"existed": true, "legacy": "refused"}));
        assert_eq!(reply.note.as_deref(), Some("legacy=refused"));
        assert!(!reply.header.to_string().contains("128"), "kind only");
        assert!(removed.lock().unwrap().is_empty());
        assert_eq!(vault_of(&dir).get("voyage").unwrap(), None);

        // The old item survives, but the marker keeps it from coming back.
        assert_eq!(
            call(&state, "has", json!({"secret": "voyage"})).unwrap()["has"],
            false
        );
        let (fresh, _) = state_over(&dir, items(vec![(VOYAGE, "pa-old")]));
        assert_eq!(
            call(&fresh, "has", json!({"secret": "voyage"})).unwrap()["has"],
            false
        );
    }

    #[test]
    fn a_failed_removal_is_reported_as_failed() {
        let dir = Scratch::new("legacy-failed");
        let (state, _) = state_over(
            &dir,
            items(vec![(VOYAGE, "pa-old")]).failing_removal(KeyError::Platform("boom".into())),
        );
        let reply = call(&state, "delete", json!({"secret": "voyage"})).unwrap();
        assert_eq!(reply["legacy"], "failed");
        assert!(!reply.to_string().contains("boom"));
    }

    #[test]
    fn forgetting_okta_removes_the_three_old_items() {
        let dir = Scratch::new("legacy-okta");
        let (state, removed) = state_over(
            &dir,
            items(vec![
                ((SSO, "username"), "u"),
                ((SSO, "password"), "p"),
                ((SSO, "totp_secret"), "t"),
                (VOYAGE, "pa-old"),
            ]),
        );
        let reply = op_as(&state, "okta_forget");
        assert_eq!(reply, json!({"existed": false, "legacy": "removed"}));
        let mut gone = removed.lock().unwrap().clone();
        gone.sort();
        assert_eq!(
            gone,
            ["password", "totp_secret", "username"]
                .map(|a| (SSO.to_string(), a.to_string()))
                .to_vec()
        );
        for name in names::OKTA {
            assert!(vault_of(&dir)
                .get(&names::imported(name))
                .unwrap()
                .is_some());
        }
        assert_eq!(op_as(&state, "okta_forget")["legacy"], "absent");
    }

    #[test]
    fn a_refusal_on_the_first_okta_item_stops_the_rest() {
        let dir = Scratch::new("legacy-okta-refused");
        let (state, removed) = state_over(
            &dir,
            items(vec![((SSO, "username"), "u")])
                .failing_removal(KeyError::Refused("cancelled".into())),
        );
        assert_eq!(op_as(&state, "okta_forget")["legacy"], "refused");
        assert!(removed.lock().unwrap().is_empty());
    }

    #[test]
    fn the_generic_ops_still_refuse_the_managed_names() {
        let dir = Scratch::new("legacy-managed");
        let (state, removed) = state_over(&dir, items(vec![((SSO, "password"), "p")]));
        for name in names::OKTA.iter().chain(names::SESSIONS) {
            let err = call_err(&state, name);
            assert_eq!(err, "request", "{name}");
        }
        assert!(removed.lock().unwrap().is_empty());
    }

    fn call_err(state: &State, name: &str) -> &'static str {
        state
            .dispatch(&cli(), "delete", &json!({"secret": name}), b"")
            .unwrap_err()
            .kind
    }
}
