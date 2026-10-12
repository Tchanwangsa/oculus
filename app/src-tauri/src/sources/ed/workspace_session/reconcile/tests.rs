use std::collections::BTreeMap;
use std::io::Write;

use super::compare::{run, Conflict, Head, Remote};
use super::server_copy::{is_copy, name, original};
use super::table::{decide, Action};
use crate::sources::ed::workspace_session::context::sha256;
use crate::sources::ed::workspace_state::WorkspaceState;
use crate::test_support::Scratch;

#[test]
fn the_decision_table() {
    use Action::*;
    let (a, b, c) = (Some("a"), Some("b"), Some("c"));
    // (local, base, server) → action
    let table = [
        // In step, or both moved to the same bytes.
        (a, a, a, Nothing),
        (a, b, a, Record),
        (a, None, a, Record),
        (None, None, None, Nothing),
        (None, a, None, Record),
        // One side edited.
        (a, a, b, Take),
        (b, a, a, Push),
        // Both edited, differently.
        (b, a, c, Conflict),
        (a, None, b, Conflict),
        // Only on one side.
        (a, None, None, Push),
        (None, None, a, Take),
        (None, a, a, Remove),
        (a, a, None, DeleteLocal),
        // A delete loses to an edit on the other side.
        (b, a, None, Push),
        (None, a, b, Take),
    ];
    for (local, base, server, action) in table {
        assert_eq!(
            decide(local, base, server),
            action,
            "local {local:?}, base {base:?}, server {server:?}"
        );
    }
}

#[test]
fn a_server_copy_goes_before_the_last_extension() {
    assert_eq!(name("a.py"), "a.ed-server.py");
    assert_eq!(name("Makefile"), "Makefile.ed-server");
    assert_eq!(name("dir/x.tar.gz"), "dir/x.tar.ed-server.gz");
    assert_eq!(name("src/my.module.py"), "src/my.module.ed-server.py");
    for path in ["a.py", "Makefile", "dir/x.tar.gz", "src/my.module.py"] {
        assert_eq!(original(&name(path)).as_deref(), Some(path));
        assert!(is_copy(&name(path)));
        assert!(!is_copy(path));
    }
    assert!(!is_copy("a.py.ed-server"));
    assert!(!is_copy("ed-server.py"));
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        w.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// Ed in memory: its tree, and what reconcile did to it.
#[derive(Default)]
struct FakeEd {
    tree: BTreeMap<String, Vec<u8>>,
    downloads: usize,
    uploads: Vec<String>,
    removes: Vec<String>,
}

impl Remote for FakeEd {
    fn tree(&mut self) -> Result<Vec<u8>, String> {
        self.downloads += 1;
        let entries: Vec<(&str, &[u8])> = self
            .tree
            .iter()
            .map(|(p, b)| (p.as_str(), b.as_slice()))
            .collect();
        Ok(zip_of(&entries))
    }

    fn upload(&mut self, path: &str, bytes: &[u8]) -> Result<String, String> {
        self.tree.insert(path.to_string(), bytes.to_vec());
        self.uploads.push(path.to_string());
        Ok(format!("up{}", self.uploads.len()))
    }

    fn remove(&mut self, path: &str) -> Result<(), String> {
        self.tree.remove(path);
        self.removes.push(path.to_string());
        Ok(())
    }
}

fn record(files: &[(&str, &[u8])], base: &str) -> WorkspaceState {
    WorkspaceState {
        base_commit: Some(base.into()),
        files: files
            .iter()
            .map(|(p, b)| (p.to_string(), sha256(b)))
            .collect(),
        ..WorkspaceState::default()
    }
}

const CHANGED: Head = Head {
    hash: Some("h1"),
    changed: true,
};

#[test]
fn reconcile_does_what_the_table_says_and_settles() {
    let dir = Scratch::new("ed-reconcile-run");
    let root = dir.join("ws");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let base = record(
        &[
            ("same.py", b"s"),
            ("mine.py", b"m0"),
            ("theirs.py", b"t0"),
            ("both.py", b"b0"),
            ("gone_here.py", b"g"),
            ("gone_there.py", b"x"),
        ],
        "h0",
    );
    for (p, b) in [
        ("same.py", "s"),
        ("mine.py", "m1"),
        ("theirs.py", "t0"),
        ("both.py", "b-mine"),
        ("gone_there.py", "x"),
        ("src/new_here.py", "n"),
    ] {
        std::fs::write(root.join(p), b).unwrap();
    }
    let mut ed = FakeEd::default();
    for (p, b) in [
        ("same.py", "s"),
        ("mine.py", "m0"),
        ("theirs.py", "t1"),
        ("both.py", "b-ed"),
        ("gone_here.py", "g"),
        ("new_there.py", "e"),
    ] {
        ed.tree.insert(p.into(), b.as_bytes().to_vec());
    }

    let out = run(&root, &base, CHANGED, &mut ed).unwrap();

    let read = |p: &str| std::fs::read_to_string(root.join(p)).ok();
    assert_eq!(read("theirs.py").as_deref(), Some("t1"));
    assert_eq!(read("new_there.py").as_deref(), Some("e"));
    assert_eq!(read("gone_there.py"), None);
    // The conflict keeps the local file and puts Ed's beside it.
    assert_eq!(read("both.py").as_deref(), Some("b-mine"));
    assert_eq!(read("both.ed-server.py").as_deref(), Some("b-ed"));
    assert_eq!(
        out.conflicts,
        [Conflict {
            path: "both.py".into(),
            server_path: "both.ed-server.py".into()
        }]
    );
    assert_eq!(ed.uploads, ["mine.py", "src/new_here.py"]);
    assert_eq!(ed.removes, ["gone_here.py"]);
    assert_eq!(ed.tree["both.py"], b"b-ed");
    assert_eq!(out.base_commit.as_deref(), Some("up2"));
    assert_eq!(
        out.changed_here.iter().collect::<Vec<_>>(),
        [
            "both.ed-server.py",
            "gone_there.py",
            "new_there.py",
            "theirs.py"
        ]
    );
    // The conflicted file keeps its base; a remove stays until confirmed.
    assert_eq!(out.files["both.py"], sha256(b"b0"));
    assert_eq!(out.files["gone_here.py"], sha256(b"g"));
    assert!(!out.files.contains_key("gone_there.py"));
    assert!(!out.files.keys().any(|p| is_copy(p)));

    // A second run uploads nothing (the conflict waits), refreshes nothing,
    // and forgets the confirmed remove.
    let again = WorkspaceState {
        files: out.files.clone(),
        base_commit: out.base_commit.clone(),
        ..WorkspaceState::default()
    };
    let out2 = run(&root, &again, CHANGED, &mut ed).unwrap();
    assert_eq!(ed.uploads.len(), 2);
    assert_eq!(ed.removes.len(), 1);
    assert!(out2.changed_here.is_empty());
    assert!(!out2.files.contains_key("gone_here.py"));
    assert_eq!(out2.conflicts.len(), 1);
}

#[test]
fn an_unchanged_head_skips_the_download_and_only_pushes() {
    let dir = Scratch::new("ed-reconcile-short");
    let root = dir.join("ws");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.py"), "a1").unwrap();
    std::fs::write(root.join("keep.py"), "k").unwrap();
    let base = record(
        &[("a.py", b"a0"), ("keep.py", b"k"), ("gone.py", b"g")],
        "h0",
    );
    let mut ed = FakeEd::default();

    let head = Head {
        hash: Some("h0"),
        changed: false,
    };
    let out = run(&root, &base, head, &mut ed).unwrap();
    assert_eq!(ed.downloads, 0);
    assert_eq!(ed.uploads, ["a.py"]);
    assert_eq!(ed.removes, ["gone.py"]);
    assert_eq!(out.base_commit.as_deref(), Some("up1"));

    // An owed pull downloads even at the base commit.
    let owed = Head {
        hash: Some("h0"),
        changed: true,
    };
    run(&root, &base, owed, &mut ed).unwrap();
    assert_eq!(ed.downloads, 1);
}

#[test]
fn a_file_ed_left_out_of_the_zip_is_not_taken_for_deleted() {
    let dir = Scratch::new("ed-reconcile-unknown");
    let root = dir.join("ws");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a:b.txt"), "x").unwrap();
    let base = record(&[("a:b.txt", b"x")], "h0");
    let mut ed = FakeEd::default();
    ed.tree.insert("a:b.txt".into(), b"x".to_vec());
    ed.tree.insert("x.ed-server.py".into(), b"y".to_vec());

    let out = run(&root, &base, CHANGED, &mut ed).unwrap();
    assert!(root.join("a:b.txt").exists());
    assert!(!root.join("x.ed-server.py").exists());
    assert_eq!(out.files["a:b.txt"], sha256(b"x"));
    assert_eq!(out.skipped.len(), 2, "{:?}", out.skipped);
}

#[test]
fn a_missing_folder_is_an_error_not_a_delete_of_everything() {
    let dir = Scratch::new("ed-reconcile-missing");
    let mut ed = FakeEd::default();
    ed.tree.insert("a.py".into(), b"a".to_vec());
    let base = record(&[("a.py", b"a")], "h0");
    assert!(run(&dir.join("nope"), &base, CHANGED, &mut ed).is_err());
    assert!(ed.removes.is_empty());
}
