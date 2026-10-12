use super::super::stress::{apply_stress, stress_weight};

#[test]
fn none_leaves_the_phonemes() {
    assert_eq!(apply_stress("həlˈO", None), "həlˈO");
}

#[test]
fn below_minus_one_strips_every_mark() {
    assert_eq!(apply_stress("ˌɛmˈɛl", Some(-2.0)), "ɛmɛl");
}

#[test]
fn minus_one_demotes_primaries() {
    assert_eq!(apply_stress("ˌɛmˈɛl", Some(-1.0)), "ɛmˌɛl");
    assert_eq!(apply_stress("ˈæpᵊl", Some(-0.5)), "ˌæpᵊl");
}

#[test]
fn a_mark_is_added_before_the_first_vowel() {
    assert_eq!(apply_stress("kæt", Some(0.5)), "kˌæt");
    assert_eq!(apply_stress("kæt", Some(2.0)), "kˈæt");
    assert_eq!(apply_stress("bʤ", Some(2.0)), "bʤ");
}

#[test]
fn a_lone_secondary_is_promoted() {
    assert_eq!(apply_stress("ˈIfˌOn", Some(1.0)), "ˈIfˌOn");
    assert_eq!(apply_stress("ɪfˌOn", Some(2.0)), "ɪfˈOn");
}

#[test]
fn diphthongs_weigh_two() {
    assert_eq!(stress_weight("ʤˈA"), 5);
    assert_eq!(stress_weight(""), 0);
}
