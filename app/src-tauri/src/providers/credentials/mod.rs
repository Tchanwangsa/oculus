//! `Credentialed`, the client of `oculus-keyd` (`keyd_core::client`), and
//! `CloudKey`, the Settings side of the three cloud keys keyd holds. The
//! keychain items are the fallback while keyd is absent (`keychain.rs`).
//! Callers own validation and probing; values never leave keyd or the
//! keychain here.

mod keychain;

pub(crate) use keychain::Secret;
pub(crate) use keyd_core::client::{Client as Credentialed, KeydError, RawResponse};

/// A cloud key (Voyage, MinerU, Groq) as Settings stores it: in keyd's vault
/// under `secret` when keyd is installed, else in the `keychain` item. Only
/// `KeydError::Absent` falls back to the keychain; every other keyd error
/// surfaces. `what` is the name the settings page gives the key.
pub(crate) struct CloudKey {
    pub(crate) secret: &'static str,
    pub(crate) what: &'static str,
    pub(crate) keychain: Secret<'static>,
}

impl CloudKey {
    /// The keychain item's value, for the path that runs with keyd absent.
    /// `Err` when the keychain refused, as opposed to holding no key.
    pub(crate) fn fetch(&self) -> Result<Option<String>, String> {
        self.keychain.fetch()
    }

    /// `store_checked`: `probe` runs here with the key just typed; only the
    /// store goes through keyd.
    pub(crate) fn set(
        &self,
        key: &str,
        probe: impl FnOnce(&str) -> Result<Verdict, String>,
    ) -> Result<String, String> {
        let broker = Credentialed::at(&crate::library::paths::data_dir());
        store_checked(key, probe, |value| {
            self.store_in(&broker, value, |v| self.keychain.write(v))
        })
    }

    pub(crate) fn has(&self) -> Result<bool, String> {
        self.has_in(
            &Credentialed::at(&crate::library::paths::data_dir()),
            || self.keychain.has(self.what),
        )
    }

    pub(crate) fn delete(&self) -> Result<(), String> {
        self.delete_in(
            &Credentialed::at(&crate::library::paths::data_dir()),
            || self.keychain.delete(),
        )
    }

    /// Each `*_in` asks `broker` and uses `keychain` only when keyd is absent.
    fn store_in(
        &self,
        broker: &Credentialed,
        value: &str,
        keychain: impl FnOnce(&str) -> Result<(), String>,
    ) -> Result<(), String> {
        match broker.store(self.secret, value) {
            Err(KeydError::Absent) => keychain(value),
            other => other.map_err(|e| e.to_string()),
        }
    }

    fn has_in(
        &self,
        broker: &Credentialed,
        keychain: impl FnOnce() -> Result<bool, String>,
    ) -> Result<bool, String> {
        let what = self.what;
        match broker.has(self.secret) {
            Err(KeydError::Absent) => keychain(),
            Err(KeydError::Keychain(e)) => {
                Err(format!("The keychain refused to give out the {what} ({e})"))
            }
            other => other.map_err(|e| format!("Could not check the {what}: {e}")),
        }
    }

    fn delete_in(
        &self,
        broker: &Credentialed,
        keychain: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        match broker.delete(self.secret) {
            Err(KeydError::Absent) => keychain(),
            other => other.map(|_| ()).map_err(|e| e.to_string()),
        }
    }
}

/// Probe before storing. An unreachable service can accept a key on trust;
/// an active refusal leaves the saved key untouched. `write` stores the
/// trimmed key: into the keychain, or through keyd.
pub(crate) fn store_checked(
    key: &str,
    probe: impl FnOnce(&str) -> Result<Verdict, String>,
    write: impl FnOnce(&str) -> Result<(), String>,
) -> Result<String, String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("empty key".into());
    }
    let verdict = probe(key)?;
    write(key)?;
    Ok(match verdict {
        Verdict::Good => "ok".into(),
        Verdict::Unverified => "unverified".into(),
    })
}

/// What a probe learned about a key. `Unverified`: the service was unreachable,
/// or answered about the account rather than the key.
pub(crate) enum Verdict {
    Good,
    Unverified,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeKeyd, Scratch};
    use serde_json::json;

    #[test]
    fn empty_keys_are_rejected_before_probing_or_touching_the_keychain() {
        assert_eq!(
            store_checked(
                " \n\t ",
                |_| panic!("must not probe"),
                |_| panic!("must not write")
            )
            .unwrap_err(),
            "empty key"
        );
    }

    #[test]
    fn probes_receive_trimmed_keys_and_refusal_prevents_storage() {
        let result = store_checked(
            "  pa-key \n",
            |value| {
                assert_eq!(value, "pa-key");
                Err("refused".into())
            },
            |_| panic!("must not write"),
        );
        assert_eq!(result.unwrap_err(), "refused");
    }

    /// The three keys Settings stores, by the name keyd knows each by.
    fn cloud_keys() -> [(&'static CloudKey, &'static str); 3] {
        [
            (&crate::providers::voyage::KEY, "voyage"),
            (&crate::providers::mineru::KEY, "mineru"),
            (&crate::providers::groq::KEY, "groq"),
        ]
    }

    #[test]
    fn with_keyd_installed_the_keychain_is_never_touched() {
        for (key, name) in cloud_keys() {
            assert_eq!(key.secret, name);
            let dir = Scratch::new("cloudkey-keyd");
            let keyd = FakeKeyd::start(&dir, |req, _| match req["op"].as_str().unwrap() {
                "has" => (json!({"has": true}), vec![]),
                "store" => (json!({"stored": true}), vec![]),
                _ => (json!({"existed": true}), vec![]),
            });
            let broker = Credentialed::at(&dir);
            assert!(key.has_in(&broker, || panic!("keychain read")).unwrap());
            let stored = store_checked(
                " new-key\n",
                |_| Ok(Verdict::Good),
                |v| key.store_in(&broker, v, |_| panic!("keychain write")),
            );
            assert_eq!(stored.unwrap(), "ok");
            key.delete_in(&broker, || panic!("keychain delete"))
                .unwrap();
            assert_eq!(keyd.ops(), ["has", "store", "delete"], "{name}");
            for (header, _) in keyd.requests() {
                assert_eq!(header["secret"], name);
            }
            assert_eq!(keyd.requests()[1].0["value"], "new-key", "the trimmed key");
        }
    }

    #[test]
    fn a_refused_probe_never_reaches_keyd() {
        for (key, _) in cloud_keys() {
            let dir = Scratch::new("cloudkey-probe");
            let keyd = FakeKeyd::start(&dir, |_, _| (json!({"stored": true}), vec![]));
            let broker = Credentialed::at(&dir);
            let result = store_checked(
                "bad-key",
                |_| Err("refused".into()),
                |v| key.store_in(&broker, v, |_| panic!("keychain write")),
            );
            assert_eq!(result.unwrap_err(), "refused");
            assert!(keyd.requests().is_empty());
        }
    }

    #[test]
    fn without_keyd_the_keychain_answers() {
        for (key, _) in cloud_keys() {
            let dir = Scratch::new("cloudkey-absent");
            let broker = Credentialed::at(&dir);
            assert!(!key.has_in(&broker, || Ok(false)).unwrap());
            let wrote = std::cell::Cell::new(false);
            key.store_in(&broker, "x", |_| {
                wrote.set(true);
                Ok(())
            })
            .unwrap();
            assert!(wrote.get());
            assert_eq!(
                key.delete_in(&broker, || Err("denied".into())).unwrap_err(),
                "denied"
            );
        }
    }

    #[test]
    fn keyds_refusals_surface_and_never_fall_back() {
        for (key, _) in cloud_keys() {
            let dir = Scratch::new("cloudkey-refused");
            let _keyd = FakeKeyd::start(&dir, |req, _| match req["op"].as_str().unwrap() {
                "has" => (
                    json!({"error": "keychain", "detail": "OSStatus -128"}),
                    vec![],
                ),
                _ => (
                    json!({"error": "vault", "detail": "vault.bin is damaged"}),
                    vec![],
                ),
            });
            let broker = Credentialed::at(&dir);
            let err = key.has_in(&broker, || panic!("keychain read")).unwrap_err();
            assert!(
                err.starts_with(&format!(
                    "The keychain refused to give out the {}",
                    key.what
                )) && err.contains("-128"),
                "{err}"
            );
            let err = key
                .store_in(&broker, "x", |_| panic!("keychain write"))
                .unwrap_err();
            assert!(err.contains("damaged"), "{err}");
            assert!(key
                .delete_in(&broker, || panic!("keychain delete"))
                .is_err());
        }
    }
}
