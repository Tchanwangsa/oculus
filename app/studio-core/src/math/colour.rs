//! A `\color{…}` value: a theme colour's name (`\color{accent}`), else a
//! hex colour as KaTeX accepts it (`#rgb`, `#rgba`, `#rrggbb`,
//! `#rrggbbaa`, or six digits without `#`). CSS colour names are refused,
//! as everywhere in Studio: colours come from the theme. `transparent` is
//! what `\phantom` writes; its pieces are left out (`build`).

use crate::scene::Color;
use crate::suggest::did_you_mean;
use serde_json::{Map, Value};

pub fn resolve(c: &str, colours: Option<&Map<String, Value>>) -> Result<Color, String> {
    if c == "transparent" {
        return Ok(Color::CLEAR);
    }
    if let Some(v) = colours.and_then(|m| m.get(c)).and_then(Value::as_str) {
        return Color::parse(v);
    }
    let hex = c.strip_prefix('#').unwrap_or(c);
    let is_hex = !hex.is_empty() && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if is_hex && (c.starts_with('#') || hex.len() == 6) {
        let full = match hex.len() {
            4 => hex.chars().flat_map(|ch| [ch, ch]).collect(),
            _ => hex.to_owned(),
        };
        if let Ok(colour) = Color::parse(&format!("#{full}")) {
            return Ok(colour);
        }
    }
    let names: Vec<&str> = colours
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default();
    Err(format!(
        "\\color{{{c}}}: not a theme colour or a hex colour{}; theme colours: {}",
        did_you_mean(c, names.iter().copied()),
        names.join(", ")
    ))
}
