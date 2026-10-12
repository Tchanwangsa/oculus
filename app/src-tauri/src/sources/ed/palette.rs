//! The colour Ed gives a board's top-level category.

/// Mirrors Ed's discuss store: the category at position `i` in the course's
/// `settings.discussion.categories` takes `CATEGORY_COLORS[i]`.
const CATEGORY_COLORS: [&str; 16] = [
    "#3498db", "#2ecc71", "#f39c12", "#e74c3c", "#9b59b6", "#50d7ce", "#f72792", "#a1e136",
    "#686ad0", "#ff5600", "#d25efd", "#22e674", "#aa1a72", "#6fa89c", "#0058c4", "#38eab5",
];

/// `category`'s colour among the course's top-level `categories` (in Ed's
/// order). `None` past the palette or for a name not listed, where Ed shows grey.
pub fn category_color(categories: &[String], category: &str) -> Option<&'static str> {
    let i = categories.iter().position(|c| c == category)?;
    CATEGORY_COLORS.get(i).copied()
}
