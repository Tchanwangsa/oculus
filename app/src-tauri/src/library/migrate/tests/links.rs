use crate::library::migrate::links::Linker;
use crate::library::migrate::links::{encode, normalize, percent_decode};
use crate::library::migrate::rules::{Rule, Rules};
use crate::test_support::Scratch;

use super::fixture::file;

fn rules(pairs: &[(&str, &str)]) -> Rules {
    let list: Vec<Rule> = pairs
        .iter()
        .map(|(old, new)| Rule {
            old: old.to_string(),
            new: new.to_string(),
        })
        .collect();
    Rules::new(&list)
}

#[test]
fn links_follow_their_targets_in_the_spelling_the_sync_writes_and_a_second_pass_is_a_no_op() {
    let d = Scratch::new("migrate-links");
    file(&d, "courses/X/Course/Files/a b.pdf", "pdf");
    file(&d, "courses/X/Course/Images/7.png", "png");
    file(&d, "courses/X/Course/Modules/01.md", "");
    file(&d, "courses/X/.assets/p.png", "png");
    let r = rules(&[
        ("courses/X/files", "courses/X/Course/Files"),
        ("courses/X/images", "courses/X/Course/Images"),
        ("courses/X/modules", "courses/X/Course/Modules"),
        ("courses/X/pages", "courses/X/Course/Pages"),
        ("courses/X/home.md", "courses/X/Course/home.md"),
        ("courses/X/documents", "courses/X"),
        ("courses/X/documents/assets", "courses/X/.assets"),
    ]);
    let linker = Linker::new(&d, &r);
    let toc =
        "[a](../files/a%20b.pdf#page=2) [m](missing.md) [w](https://x.org/a) [i](../images/7.png) \
               [v](../files/talk.mp4) [p](../pages/later.md)";
    let (out, n) = linker.relink(
        toc,
        "courses/X/modules/01.md",
        "courses/X/Course/Modules/01.md",
    );
    assert_eq!(
        out,
        "[a](../Files/a%20b.pdf#page=2) [m](missing.md) [w](https://x.org/a) [i](../Images/7.png) \
         [v](../Media/talk.mp4) [p](../Pages/later.md)"
    );
    assert_eq!(
        n, 4,
        "a video or page not fetched yet is linked where it will land"
    );
    let (again, n) = linker.relink(
        &out,
        "courses/X/modules/01.md",
        "courses/X/Course/Modules/01.md",
    );
    assert_eq!((again.as_str(), n), (out.as_str(), 0));

    let (home, _) = linker.relink(
        "![](images/7.png)",
        "courses/X/home.md",
        "courses/X/Course/home.md",
    );
    assert_eq!(home, "![](Images/7.png)");

    let (note, _) = linker.relink(
        "![](assets/p.png) [a](<../files/a b.pdf>)",
        "courses/X/documents/n.md",
        "courses/X/n.md",
    );
    assert_eq!(note, "![](.assets/p.png) [a](<Course/Files/a b.pdf>)");
}

#[test]
fn an_absolute_asset_link_moves_into_this_data_dir() {
    let d = Scratch::new("migrate-asset-links");
    let r = rules(&[("courses/X/files", "courses/X/Course/Files")]);
    let linker = Linker::new(&d, &r);
    let text = "![](asset://localhost/%2Fold%2Fdata%2Fcourses%2FX%2Ffiles%2Fa_images%2F1.jpg)";
    let (out, n) = linker.relink(text, "courses/X/n.md", "courses/X/n.md");
    assert_eq!(n, 1);
    let want = format!(
        "{}/courses/X/Course/Files/a_images/1.jpg",
        crate::library::paths::slash_path(&*d)
    );
    assert_eq!(
        out,
        format!("![](asset://localhost/{})", encode(&want, false))
    );
    assert_eq!(linker.relink(&out, "courses/X/n.md", "courses/X/n.md").1, 0);
}

#[test]
fn helpers() {
    assert_eq!(normalize("a/b", "../c/./d").as_deref(), Some("a/c/d"));
    assert_eq!(normalize("a", "../../x"), None);
    assert_eq!(percent_decode("a%20b%2F").as_deref(), Some("a b/"));
    assert_eq!(encode("/a b/c(1).png", false), "%2Fa%20b%2Fc(1).png");
}
