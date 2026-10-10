//! Namespace management with TeX-like grouping semantics
//!
//! This is a Rust port of KaTeX's `src/Namespace.js`.
//! A `Namespace` refers to a space of nameable things like macros or lengths,
//! which can be set either globally or local to a nested group using an
//! undo stack similar to how TeX implements this functionality.

use core::cell::RefMut;

use rapidhash::{RapidHashMap, RapidHashSet};

use crate::types::{ParseError, ParseErrorKind};

/// Make it easier to switch between different hash backends.
pub type KeyMap<K, V> = RapidHashMap<K, V>;
/// Alias for the default hash set.
pub type KeySet<K> = RapidHashSet<K>;
/// Mapping type alias
pub type Mapping<V> = KeyMap<String, V>;

/// A node's attributes, kept in insertion order: KaTeX JS writes them in the
/// order they were set (a JS object's key order), so markup must too. Setting
/// an existing key replaces its value in place, as a JS object does. Nodes
/// carry a handful of attributes, so lookups are a linear scan.
#[derive(Clone, Default)]
pub struct AttrMap(Vec<(String, String)>);

impl core::fmt::Debug for AttrMap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl AttrMap {
    /// An empty map.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Sets `key`, keeping its position if already present; returns the old
    /// value.
    pub fn insert(&mut self, key: String, value: String) -> Option<String> {
        if let Some((_, v)) = self.0.iter_mut().find(|(k, _)| *k == key) {
            return Some(core::mem::replace(v, value));
        }
        self.0.push((key, value));
        None
    }

    /// The value of `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value of `key`, mutably.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut String> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Whether `key` is set.
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.iter().any(|(k, _)| k == key)
    }

    /// Removes `key`, keeping the others' order.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        let i = self.0.iter().position(|(k, _)| k == key)?;
        Some(self.0.remove(i).1)
    }

    /// The attributes in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.0.iter().map(|(k, v)| (k, v))
    }

    /// The number of attributes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no attributes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Equal when both hold the same pairs, in any order (as the hash map was).
impl PartialEq for AttrMap {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl Extend<(String, String)> for AttrMap {
    fn extend<T: IntoIterator<Item = (String, String)>>(&mut self, iter: T) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl FromIterator<(String, String)> for AttrMap {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl<const N: usize> From<[(String, String); N]> for AttrMap {
    fn from(pairs: [(String, String); N]) -> Self {
        pairs.into_iter().collect()
    }
}

impl IntoIterator for AttrMap {
    type Item = (String, String);
    type IntoIter = alloc::vec::IntoIter<(String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a AttrMap {
    type Item = (&'a String, &'a String);
    type IntoIter = core::iter::Map<
        core::slice::Iter<'a, (String, String)>,
        fn(&'a (String, String)) -> (&'a String, &'a String),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(k, v)| (k, v))
    }
}

/// A `Namespace` implements scoped definitions with begin/end group semantics.
///
/// Performance characteristics mirror the JS version:
/// - `get` and local `set` are O(1)
/// - global `set` is O(depth), where depth is the group nesting level
#[derive(Debug)]
pub struct Namespace<'a, V: Clone + 'static> {
    /// Current (mutable) mapping. Represents the global-level table that
    /// local changes modify, with undos recorded on the stack.
    current: RefMut<'a, Mapping<V>>,
    /// Built-in immutable mappings that never change.
    builtins: &'static phf::Map<&'static str, V>,
    /// Stack of undo maps for nested groups. The stored value is the previous
    /// value of a name (or `None` to indicate deletion) to restore on pop.
    undef_stack: Vec<KeyMap<String, Option<V>>>,
}

impl<'a, V: Clone> Namespace<'a, V> {
    /// Create a new namespace.
    /// - `builtins` are immutable defaults
    /// - `global` initializes the mutable global mapping
    #[must_use]
    pub const fn new(
        builtins: &'static phf::Map<&'static str, V>,
        global: RefMut<'a, Mapping<V>>,
    ) -> Self {
        Self {
            current: global,
            builtins,
            undef_stack: Vec::new(),
        }
    }

    /// Start a new nested group, affecting future local `set`s.
    pub fn begin_group(&mut self) {
        self.undef_stack.push(KeyMap::default());
    }

    /// Purge any key from current
    pub fn purge(&mut self, name: &str) {
        self.current.remove(name);
    }

    fn restore_changes<I>(&mut self, undefs: I)
    where
        I: IntoIterator<Item = (String, Option<V>)>,
    {
        for (name, previous) in undefs {
            match previous {
                Some(v) => {
                    self.current.insert(name, v);
                }
                None => {
                    self.current.remove(&name);
                }
            }
        }
    }

    /// End current nested group, restoring values before the group began.
    pub fn end_group(&mut self) -> Result<(), ParseError> {
        let undefs = self
            .undef_stack
            .pop()
            .ok_or_else(|| ParseError::new(ParseErrorKind::UnbalancedNamespaceDestruction))?;
        self.restore_changes(undefs);
        Ok(())
    }

    /// Ends all currently nested groups (if any), restoring values before the
    /// groups began.
    pub fn end_groups(&mut self) -> usize {
        let mut count = 0;
        while let Some(undefs) = self.undef_stack.pop() {
            self.restore_changes(undefs);
            count += 1;
        }
        count
    }

    /// Detect whether `name` has a definition (either current or builtin)
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.current.contains_key(name) || self.builtins.contains_key(name)
    }

    /// Get the current value of a name, or `None` if there is no value.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&V> {
        self.current.get(name).or_else(|| self.builtins.get(name))
    }

    /// Set the current value of a name, and optionally set it globally too.
    ///
    /// Local `set` sets the current value and (when appropriate) adds an undo
    /// operation to the undo stack. Global `set` may change the undo operation
    /// at every level, so takes time linear in the number of nested groups.
    /// A value of `None` means to delete existing definitions.
    /// In JavaScript, `global` is optional and defaults to `false`.
    pub fn set(&mut self, name: &str, value: Option<V>, global: bool) {
        if global {
            // Global set is equivalent to setting in all groups. Simulate this
            // by destroying any undos currently scheduled for this name, and
            // adding an undo with the new value (in case it later gets locally
            // reset within this environment).
            for level in &mut self.undef_stack {
                level.remove(name);
            }
            if let Some(top) = self.undef_stack.last_mut() {
                top.insert(name.to_owned(), value.clone());
            }
        } else {
            // Undo this set at end of this group (possibly to `None`), unless
            // an undo is already in place, in which case that older value is
            // the correct one.
            if let Some(top) = self.undef_stack.last_mut()
                && !top.contains_key(name)
            {
                let prev = self.current.get(name).cloned();
                top.insert(name.to_owned(), prev);
            }
        }

        match value {
            Some(v) => {
                self.current.insert(name.to_owned(), v);
            }
            None => {
                self.current.remove(name);
            }
        }
    }
}
