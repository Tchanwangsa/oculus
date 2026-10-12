//! The cascade's rules: the first source by priority wins a name, the names
//! are the union, and a miss lists every name.
#![allow(clippy::unwrap_used, clippy::panic)]

use super::{CascadeComponents, ComponentSource};
use crate::error::{Result, StudioError};
use std::sync::Arc;

/// Components held in memory, as a folder would hold them.
struct Mem(Vec<(&'static str, &'static str)>);

impl ComponentSource for Mem {
    fn read(&self, name: &str) -> Result<String> {
        self.0
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, src)| (*src).to_owned())
            .ok_or_else(|| StudioError::ComponentNotFound {
                name: name.to_owned(),
                message: "not in this source".to_owned(),
            })
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.0.iter().map(|(n, _)| (*n).to_owned()).collect();
        names.sort();
        names
    }
}

fn cascade() -> CascadeComponents {
    // Project, then subject, then global.
    CascadeComponents::new(vec![
        (
            "project".to_owned(),
            Arc::new(Mem(vec![
                ("Steps", "project steps"),
                ("Only", "project only"),
            ])),
        ),
        (
            "subject S".to_owned(),
            Arc::new(Mem(vec![
                ("Steps", "subject steps"),
                ("Card", "subject card"),
            ])),
        ),
        (
            "global".to_owned(),
            Arc::new(Mem(vec![
                ("Steps", "global steps"),
                ("Card", "global card"),
                ("Title", "global title"),
            ])),
        ),
    ])
}

#[test]
fn the_first_source_by_priority_wins_a_name() {
    let c = cascade();
    assert_eq!(c.read("Steps").unwrap(), "project steps");
    assert_eq!(c.read("Card").unwrap(), "subject card");
    assert_eq!(c.read("Title").unwrap(), "global title");
}

#[test]
fn each_name_reports_the_level_it_resolves_from() {
    assert_eq!(
        cascade().levels(),
        [
            ("Card".to_owned(), "subject S".to_owned()),
            ("Only".to_owned(), "project".to_owned()),
            ("Steps".to_owned(), "project".to_owned()),
            ("Title".to_owned(), "global".to_owned()),
        ]
    );
}

#[test]
fn names_are_the_sorted_union() {
    assert_eq!(cascade().names(), ["Card", "Only", "Steps", "Title"]);
}

#[test]
fn a_miss_lists_the_union() {
    let err = cascade().read("Ttle").unwrap_err();
    let StudioError::ComponentNotFound { name, message } = &err else {
        panic!("{err:?}")
    };
    assert_eq!(name, "Ttle");
    assert!(
        message.starts_with("no file Ttle.tsx in the project, subject or global library"),
        "{message}"
    );
    assert!(message.contains("did you mean `Title`?"), "{message}");
    assert!(
        message.ends_with("available: Card, Only, Steps, Title"),
        "{message}"
    );
}

#[test]
fn an_invalid_name_gets_the_naming_rule() {
    let err = cascade().read("lower-case").unwrap_err();
    assert!(
        err.to_string().contains("PascalCase letters and digits"),
        "{err}"
    );
}
