//! The `.canvas` validator, a line-for-line port of `validate` in
//! `app/src/lib/canvas/model.ts` and `isCanvasImageDataUrl` in
//! `imageSource.ts`. It reads a `serde_json::Value`, so unknown fields are
//! never dropped. The shared fixtures under `app/tests/fixtures/canvas-model/`
//! hold both implementations to the same answers. A present-but-`null` optional
//! field counts as present, as `!== undefined` does in TypeScript.

use serde_json::{Map, Value};
use std::collections::HashSet;

const TYPES: [&str; 8] = [
    "rectangle",
    "ellipse",
    "diamond",
    "line",
    "arrow",
    "freedraw",
    "text",
    "image",
];
const ARROWHEADS: &[&str] = &["none", "arrow", "triangle", "dot", "bar", "diamond"];
const CHOICES: [(&str, &[&str]); 8] = [
    ("strokeStyle", &["solid", "dashed", "dotted"]),
    ("fontFamily", &["sans", "serif", "mono"]),
    ("textAlign", &["left", "center", "right"]),
    ("verticalAlign", &["top", "middle", "bottom"]),
    ("startArrowhead", ARROWHEADS),
    ("endArrowhead", ARROWHEADS),
    ("roundness", &["sharp", "round"]),
    ("arrowType", &["sharp", "curved", "elbow"]),
];

/// serde_json cannot hold NaN or infinity, so every number is finite.
fn finite(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64)
}

fn unit(value: Option<&Value>) -> bool {
    finite(value).is_some_and(|v| (0.0..=1.0).contains(&v))
}

fn binding(value: &Value) -> bool {
    let Some(v) = value.as_object() else {
        return false;
    };
    let element = v.get("elementId").and_then(Value::as_str);
    if element.is_none_or(str::is_empty) {
        return false;
    }
    match v.get("mode").and_then(Value::as_str) {
        Some("center") => true,
        Some("edge") => v
            .get("anchor")
            .and_then(Value::as_object)
            .is_some_and(|a| unit(a.get("x")) && unit(a.get("y"))),
        _ => false,
    }
}

fn fixed_segment(value: &Value) -> bool {
    value.as_object().is_some_and(|v| {
        v.len() == 1 && (finite(v.get("x")).is_some() || finite(v.get("y")).is_some())
    })
}

/// A whole document, as `parseCanvasDocument` accepts it.
pub fn validate(value: &Value) -> Result<(), String> {
    let Some(doc) = value.as_object() else {
        return Err("This file is not an Oculus canvas.".into());
    };
    if doc.get("type").and_then(Value::as_str) != Some("oculus-canvas") {
        return Err("This file is not an Oculus canvas.".into());
    }
    if finite(doc.get("version")) != Some(1.0) {
        return Err("Unsupported canvas version.".into());
    }
    let Some(elements) = doc.get("elements").and_then(Value::as_array) else {
        return Err("Canvas elements must be an array.".into());
    };
    validate_viewport(doc.get("viewport"))?;
    if !doc.get("background").is_some_and(Value::is_string) {
        return Err("Canvas background must be a colour.".into());
    }
    let mut ids = HashSet::new();
    for (i, el) in elements.iter().enumerate() {
        validate_element(el, i, &mut ids)?;
    }
    Ok(())
}

/// The viewport rule `validate` applies; desktop viewport hints use it too.
pub fn validate_viewport(value: Option<&Value>) -> Result<(), String> {
    let ok = value.and_then(Value::as_object).is_some_and(|v| {
        finite(v.get("x")).is_some()
            && finite(v.get("y")).is_some()
            && finite(v.get("zoom")).is_some_and(|zoom| zoom > 0.0)
    });
    if ok {
        Ok(())
    } else {
        Err("Canvas viewport must have finite coordinates and a positive zoom.".into())
    }
}

/// One element at position `index`; `ids` collects the IDs seen so far.
pub fn validate_element<'a>(
    value: &'a Value,
    index: usize,
    ids: &mut HashSet<&'a str>,
) -> Result<(), String> {
    let n = index + 1;
    let el: &Map<String, Value> = match value.as_object() {
        Some(el) => match el.get("id").and_then(Value::as_str) {
            Some(id) if !id.is_empty() && ids.insert(id) => el,
            _ => return Err(format!("Canvas element {n} needs a unique ID.")),
        },
        None => return Err(format!("Canvas element {n} needs a unique ID.")),
    };
    let kind = el.get("type").and_then(Value::as_str);
    if !kind.is_some_and(|t| TYPES.contains(&t)) {
        return Err("Unsupported canvas element type.".into());
    }
    for key in ["x", "y", "width", "height", "strokeWidth", "seed"] {
        if finite(el.get(key)).is_none() {
            return Err(format!("Canvas element {n} has an invalid {key}."));
        }
    }
    let style = el.get("style").and_then(Value::as_str);
    if finite(el.get("strokeWidth")).is_some_and(|w| w < 0.0)
        || !el.get("stroke").is_some_and(Value::is_string)
        || !el.get("fill").is_some_and(Value::is_string)
        || !matches!(style, Some("sketch" | "clean"))
    {
        return Err(format!("Canvas element {n} has an invalid appearance."));
    }
    if el.contains_key("fontSize") && !finite(el.get("fontSize")).is_some_and(|s| s > 0.0) {
        return Err(format!("Canvas element {n} has an invalid font size."));
    }
    if el.get("text").is_some_and(|t| !t.is_string()) {
        return Err(format!("Canvas element {n} has invalid text."));
    }
    if (kind == Some("image") || el.contains_key("dataUrl"))
        && !el
            .get("dataUrl")
            .and_then(Value::as_str)
            .is_some_and(is_image_data_url)
    {
        return Err(format!(
            "Canvas element {n} needs an embedded PNG, JPEG, WebP or GIF image."
        ));
    }
    if el.get("sourceName").is_some_and(|s| !s.is_string()) {
        return Err(format!(
            "Canvas element {n} has an invalid image source name."
        ));
    }
    for key in ["naturalWidth", "naturalHeight"] {
        if el.contains_key(key) && !finite(el.get(key)).is_some_and(|v| v > 0.0) {
            return Err(format!("Canvas element {n} has an invalid {key}."));
        }
    }
    if let Some(points) = el.get("points") {
        let ok = points.as_array().is_some_and(|points| {
            points.iter().all(|p| {
                p.as_object()
                    .is_some_and(|p| finite(p.get("x")).is_some() && finite(p.get("y")).is_some())
            })
        });
        if !ok {
            return Err(format!("Canvas element {n} has invalid points."));
        }
    }
    if el.contains_key("opacity") && !unit(el.get("opacity")) {
        return Err(format!("Canvas element {n} has an invalid opacity."));
    }
    for (key, values) in CHOICES {
        if let Some(v) = el.get(key) {
            if !v.as_str().is_some_and(|v| values.contains(&v)) {
                return Err(format!("Canvas element {n} has an invalid {key}."));
            }
        }
    }
    for key in ["startBinding", "endBinding"] {
        if el.get(key).is_some_and(|b| !binding(b)) {
            return Err(format!("Canvas element {n} has an invalid {key}."));
        }
    }
    if let Some(segments) = el.get("fixedSegments") {
        if !segments
            .as_array()
            .is_some_and(|s| s.iter().all(fixed_segment))
        {
            return Err(format!("Canvas element {n} has invalid fixed segments."));
        }
    }
    Ok(())
}

/// Only embedded rasters: SVG and external URLs can reference other resources.
/// The first 32 base64 characters must decode to the format's magic bytes.
pub fn is_image_data_url(value: &str) -> bool {
    const FORMATS: [&str; 4] = ["png", "jpeg", "webp", "gif"];
    let Some(rest) = value.strip_prefix("data:image/") else {
        return false;
    };
    let Some((format, bytes)) = FORMATS.iter().find_map(|format| {
        rest.strip_prefix(format)
            .and_then(|r| r.strip_prefix(";base64,"))
            .map(|bytes| (*format, bytes.as_bytes()))
    }) else {
        return false;
    };
    if bytes.len() < 16 || bytes.len() % 4 != 0 || !base64_shape(bytes) {
        return false;
    }
    let head = decode_head(&bytes[..bytes.len().min(32)]);
    match format {
        "png" => head.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpeg" => head.starts_with(b"\xff\xd8\xff"),
        "gif" => head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a"),
        _ => head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP"),
    }
}

/// `^[A-Za-z0-9+/]+={0,2}$`
fn base64_shape(bytes: &[u8]) -> bool {
    let body = bytes.iter().position(|&b| b == b'=').unwrap_or(bytes.len());
    let padding = &bytes[body..];
    body > 0
        && padding.len() <= 2
        && padding.iter().all(|&b| b == b'=')
        && bytes[..body]
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

/// Forgiving base64 decode of an already-shape-checked prefix, as `atob` does.
fn decode_head(chars: &[u8]) -> Vec<u8> {
    let sextet = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let mut out = Vec::with_capacity(chars.len() / 4 * 3);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &c in chars.iter().take_while(|&&c| c != b'=') {
        buffer = ((buffer << 6) | u32::from(sextet(c))) & 0xff_ffff;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}
