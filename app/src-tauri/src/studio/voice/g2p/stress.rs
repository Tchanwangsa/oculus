//! Stress marks: misaki's `apply_stress` (with its `restress`) and
//! `stress_weight`, and the phoneme classes the word path tests against.

pub const PRIMARY: char = 'ˈ';
pub const SECONDARY: char = 'ˌ';

const VOWELS: &str = "AIOQWYaiuæɑɒɔəɛɜɪʊʌᵻ";
const CONSONANTS: &str = "bdfhjklmnpstvwzðŋɡɹɾʃʒʤʧθ";
const DIPHTHONGS: &str = "AIOQWYʤʧ";

pub fn is_vowel(c: char) -> bool {
    VOWELS.contains(c)
}

pub fn is_consonant(c: char) -> bool {
    CONSONANTS.contains(c)
}

fn is_stress(c: char) -> bool {
    c == PRIMARY || c == SECONDARY
}

/// A diphthong or affricate weighs two, any other character one.
pub fn stress_weight(ps: &str) -> usize {
    ps.chars()
        .map(|c| if DIPHTHONGS.contains(c) { 2 } else { 1 })
        .sum()
}

/// Every stress mark moves to just before the first vowel at or after it.
fn restress(ps: &str) -> String {
    let cs: Vec<char> = ps.chars().collect();
    let mut keyed: Vec<(f64, char)> = cs
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let key = if is_stress(c) {
                match cs[i..].iter().position(|&v| is_vowel(v)) {
                    Some(j) => (i + j) as f64 - 0.5,
                    None => i as f64,
                }
            } else {
                i as f64
            };
            (key, c)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    keyed.into_iter().map(|(_, c)| c).collect()
}

/// misaki's `apply_stress`. `stress` is one of its levels: `None` leaves
/// the phonemes alone, below -1 strips every mark, -1 (or 0 and -0.5 over a
/// primary) demotes primaries to secondaries, 0 to 1 adds a secondary to
/// unstressed phonemes, 1 and up promotes a lone secondary, above 1 adds a
/// primary to unstressed phonemes.
pub fn apply_stress(ps: &str, stress: Option<f64>) -> String {
    let Some(s) = stress else {
        return ps.to_string();
    };
    let primary = ps.contains(PRIMARY);
    let secondary = ps.contains(SECONDARY);
    let unstressed = !primary && !secondary;
    let voiced = ps.chars().any(is_vowel);
    if s < -1.0 {
        ps.replace([PRIMARY, SECONDARY], "")
    } else if s == -1.0 || ((s == 0.0 || s == -0.5) && primary) {
        ps.replace(SECONDARY, "")
            .replace(PRIMARY, &SECONDARY.to_string())
    } else if (s == 0.0 || s == 0.5 || s == 1.0) && unstressed {
        if voiced {
            restress(&format!("{SECONDARY}{ps}"))
        } else {
            ps.to_string()
        }
    } else if s >= 1.0 && !primary && secondary {
        ps.replace(SECONDARY, &PRIMARY.to_string())
    } else if s > 1.0 && unstressed {
        if voiced {
            restress(&format!("{PRIMARY}{ps}"))
        } else {
            ps.to_string()
        }
    } else {
        ps.to_string()
    }
}
