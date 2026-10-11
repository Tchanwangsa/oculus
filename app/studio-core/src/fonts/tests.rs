use super::*;
use skrifa::MetadataProvider;
use skrifa::raw::FileRef;

fn axes(bytes: &[u8]) -> Vec<(String, f32, f32)> {
    let font = match FileRef::new(bytes) {
        Ok(FileRef::Font(f)) => f,
        _ => unreachable!("bundled fonts are single fonts"),
    };
    font.axes()
        .iter()
        .map(|a| (a.tag().to_string(), a.min_value(), a.max_value()))
        .collect()
}

#[test]
fn bundled_families_are_inter_and_manrope() {
    assert_eq!(
        bundled_families(),
        vec!["Inter".to_owned(), "Manrope".to_owned()]
    );
}

#[test]
fn inter_is_variable_with_weight_and_optical_size() {
    assert_eq!(
        axes(INTER),
        vec![
            ("opsz".to_owned(), 14.0, 32.0),
            ("wght".to_owned(), 100.0, 900.0)
        ]
    );
}

#[test]
fn manrope_is_variable_in_weight() {
    assert_eq!(axes(MANROPE), vec![("wght".to_owned(), 200.0, 800.0)]);
}
