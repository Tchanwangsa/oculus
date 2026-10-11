//! A slug from a video's title: lowercase ASCII words joined by `-`, made
//! unique among the folders already under `videos/`.

use std::path::Path;

use super::valid_slug;

/// The slug a video with no title gets.
const UNTITLED: &str = "untitled";
/// Leaves room for a `-NN` suffix inside `valid_slug`'s 64 characters.
const BASE_MAX: usize = 56;

/// `Gradient descent: a primer!` -> `gradient-descent-a-primer`. Only ASCII
/// letters and digits count; any other run is one `-`. Empty when the title
/// has none.
pub fn slug_from_title(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    slug.truncate(BASE_MAX);
    slug.trim_end_matches('-').to_owned()
}

/// A slug for `title` that names no folder in `videos`: the title's own, or
/// with `-2`, `-3`... appended.
pub fn unique_slug(videos: &Path, title: Option<&str>) -> String {
    let base = Some(slug_from_title(title.unwrap_or_default()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| UNTITLED.to_owned());
    debug_assert!(valid_slug(&base));
    let taken = |slug: &str| videos.join(slug).exists();
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|slug| !taken(slug))
        .expect("an unbounded range has a free slug")
}
