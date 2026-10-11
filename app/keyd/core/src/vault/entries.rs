use std::collections::BTreeMap;
use std::fmt;

/// The decrypted map. Debug lists the names only.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Entries(pub(super) BTreeMap<String, String>);

impl Entries {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    pub fn insert(&mut self, name: &str, value: &str) {
        self.0.insert(name.to_string(), value.to_string());
    }

    pub fn remove(&mut self, name: &str) -> bool {
        self.0.remove(name).is_some()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Entries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}
