use crate::math::fonts::{Family, Font, face_files, glyph};

#[test]
fn every_face_reads_and_maps_its_characters() {
    assert_eq!(face_files().count(), 20);
    let main = glyph(Font::MAIN, 'x').unwrap();
    assert_eq!(main.face, "KaTeX_Main-Regular.ttf");
    assert!((main.advance - 0.528).abs() < 1e-3, "{}", main.advance);
    let italic = Font {
        family: Family::Math,
        bold: false,
        italic: true,
    };
    assert_eq!(glyph(italic, 'x').unwrap().face, "KaTeX_Math-Italic.ttf");
    // KaTeX_Math has only italic faces, as a browser would find.
    let upright = Font {
        italic: false,
        ..italic
    };
    assert_eq!(glyph(upright, 'x').unwrap().face, "KaTeX_Math-Italic.ttf");
    let size4 = Font {
        family: Family::Size4,
        ..Font::MAIN
    };
    assert_eq!(glyph(size4, '(').unwrap().face, "KaTeX_Size4-Regular.ttf");
    // A character its face lacks comes from KaTeX_Main.
    let ams = Font {
        family: Family::Ams,
        ..Font::MAIN
    };
    assert_eq!(glyph(ams, '=').unwrap().face, "KaTeX_Main-Regular.ttf");
    assert!(glyph(Font::MAIN, '日').is_none());
}
