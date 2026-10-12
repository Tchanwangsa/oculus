//! A component's name: a PascalCase identifier, which is also what keeps
//! `components/<Name>.tsx` inside the library.

/// The longest name; studio-core's rule has no limit, a mistake does.
const MAX_LEN: usize = 64;

pub fn check(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let pascal = chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && chars.all(|c| c.is_ascii_alphanumeric());
    if pascal && name.len() <= MAX_LEN {
        Ok(())
    } else {
        Err(format!(
            "`{name}` is not a component name: PascalCase letters and digits, like TitleCard"
        ))
    }
}

/// `components/<name>.tsx`, the path `studio_core::write_library` takes.
pub fn rel(name: &str) -> String {
    format!("components/{name}.tsx")
}
