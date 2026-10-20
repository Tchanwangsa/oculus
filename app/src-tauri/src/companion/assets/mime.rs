/// The content type for an allowed extension; any other extension is not served.
pub fn for_path(path: &str) -> Option<&'static str> {
    let extension = path.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "json" | "map" => "application/json",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::for_path;

    #[test]
    fn wasm_and_fonts_get_their_own_types() {
        assert_eq!(
            for_path("assets/geometry-1a.wasm"),
            Some("application/wasm")
        );
        assert_eq!(for_path("assets/inter.woff2"), Some("font/woff2"));
        assert_eq!(
            for_path("assets/a.JS"),
            Some("text/javascript; charset=utf-8")
        );
        assert_eq!(for_path("assets/a.exe"), None);
        assert_eq!(for_path("assets/noext"), None);
    }
}
