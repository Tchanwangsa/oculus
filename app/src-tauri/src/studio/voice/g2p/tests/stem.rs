use super::super::stem::{past, plural, progressive, stem_ed, stem_ing, stem_s};
use super::super::token::Ctx;
use super::super::Dictionary;

fn dict() -> Dictionary {
    Dictionary::from_json(
        r#"{"cat":"kˈæt","box":"bˈɑks","lady":"lˈAdi","want":"wˈɑnt","stop":"stˈɑp","run":"ɹˈʌn","make":"mˈAk","multiply":"mˈʌltəplˌI","café":"kæfˈA"}"#,
        "{}",
    )
    .unwrap()
}

#[test]
fn endings_follow_the_last_sound() {
    assert_eq!(plural("kˈæt").unwrap(), "kˈæts");
    assert_eq!(plural("bˈɑks").unwrap(), "bˈɑksᵻz");
    assert_eq!(plural("dˈɔɡ").unwrap(), "dˈɔɡz");
    assert_eq!(past("wˈɑnt").unwrap(), "wˈɑntᵻd");
    assert_eq!(past("sˈɪt").unwrap(), "sˈɪɾᵻd");
    assert_eq!(past("nˈid").unwrap(), "nˈidᵻd");
    assert_eq!(progressive("sˈɪt").unwrap(), "sˈɪɾɪŋ");
    assert_eq!(plural(""), None);
}

#[test]
fn stems_are_found_by_character_not_byte() {
    let d = dict();
    let ctx = Ctx::default();
    assert_eq!(stem_s(&d, "cats", "", None, &ctx).unwrap(), "kˈæts");
    assert_eq!(stem_s(&d, "boxes", "", None, &ctx).unwrap(), "bˈɑksᵻz");
    assert_eq!(stem_s(&d, "ladies", "", None, &ctx).unwrap(), "lˈAdiz");
    assert_eq!(stem_ed(&d, "wanted", "", None, &ctx).unwrap(), "wˈɑntᵻd");
    assert_eq!(
        stem_ed(&d, "multiplied", "", None, &ctx).unwrap(),
        "mˈʌltəplˌId"
    );
    assert_eq!(stem_ing(&d, "running", "", None, &ctx).unwrap(), "ɹˈʌnɪŋ");
    assert_eq!(stem_ing(&d, "making", "", None, &ctx).unwrap(), "mˈAkɪŋ");
    assert_eq!(stem_ing(&d, "stopping", "", None, &ctx).unwrap(), "stˈɑpɪŋ");
    assert_eq!(stem_s(&d, "cafés", "", None, &ctx).unwrap(), "kæfˈAz");
    assert_eq!(stem_ed(&d, "éé", "", None, &ctx), None);
    assert_eq!(stem_ing(&d, "ééing", "", None, &ctx), None);
}
