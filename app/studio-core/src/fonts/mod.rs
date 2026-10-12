//! Bundled fonts. Only open fonts compiled into the crate are ever used —
//! never system fonts — so a frame renders the same on every machine.
//! Licences sit beside the files in `fonts/`.

use parley::FontContext;
use parley::fontique::{Blob, Collection, CollectionOptions, FontInfoOverride, SourceCache};

/// Inter 4.1, variable (`opsz` 14-32, `wght` 100-900), SIL OFL 1.1.
const INTER: &[u8] = include_bytes!("../../fonts/Inter.ttf");

/// Manrope, variable (`wght` 200-800), SIL OFL 1.1. The app's heading face.
const MANROPE: &[u8] = include_bytes!("../../fonts/Manrope.ttf");

/// JetBrains Mono 2.304, variable (`wght` 100-800), upright only, SIL OFL 1.1.
/// The monospace face, for code.
const JETBRAINS_MONO: &[u8] = include_bytes!("../../fonts/JetBrainsMono.ttf");

/// The monospace family's name. Text set in it has ligatures off: code
/// shows `<=` and `!=` as typed.
pub const MONO_FAMILY: &str = "JetBrains Mono";

/// Family names are fixed here: Inter's own name is "Inter Variable".
const BUNDLED: &[(&str, &[u8])] = &[
    ("Inter", INTER),
    ("Manrope", MANROPE),
    (MONO_FAMILY, JETBRAINS_MONO),
];

/// A font context holding only the bundled fonts, and their family names.
pub fn font_context() -> (FontContext, Vec<String>) {
    let mut cx = FontContext {
        collection: Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: SourceCache::default(),
    };
    for (name, bytes) in BUNDLED {
        let named = FontInfoOverride {
            family_name: Some(name),
            ..FontInfoOverride::default()
        };
        cx.collection
            .register_fonts(Blob::from(bytes.to_vec()), Some(named));
    }
    (cx, bundled_families())
}

/// The bundled family names, for validating themes without a renderer.
pub fn bundled_families() -> Vec<String> {
    BUNDLED.iter().map(|(n, _)| (*n).to_owned()).collect()
}

#[cfg(test)]
mod tests;
