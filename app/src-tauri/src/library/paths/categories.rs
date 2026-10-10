/// Every category `category_from_path` can return, in reading order. The
/// `--category` flags validate against this one list; a test in `tests.rs`
/// fails if it drifts from the match.
pub const CATEGORIES: &[&str] = &[
    "home",
    "syllabus",
    "upload",
    "document",
    "page",
    "assignment",
    "quiz",
    "announcement",
    "ed",
    "file",
    "module",
    "image",
    "other",
];

pub fn category_from_path(path: &str) -> &'static str {
    match path {
        "home.md" => "home",
        "syllabus.md" => "syllabus",
        p if p.starts_with("uploads/") => "upload",
        p if p.starts_with("documents/") => "document",
        p if p.starts_with("pages/") => "page",
        p if p.starts_with("assignments/") => "assignment",
        p if p.starts_with("quizzes/") => "quiz",
        p if p.starts_with("announcements/") => "announcement",
        p if p.starts_with("ed/") => "ed",
        p if p.starts_with("files/") => "file",
        p if p.starts_with("modules/") => "module",
        p if p.starts_with("images/") => "image",
        _ => "other",
    }
}
