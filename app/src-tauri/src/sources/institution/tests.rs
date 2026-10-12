use super::domain::is_southern;
use super::*;
use crate::test_support::Scratch;

#[test]
fn normalise_keeps_only_the_origin() {
    let ok = |s: &str| normalise(s).unwrap();
    assert_eq!(
        ok("canvas.lms.unimelb.edu.au"),
        "https://canvas.lms.unimelb.edu.au"
    );
    assert_eq!(
        ok(" RMIT.instructure.com/ "),
        "https://rmit.instructure.com"
    );
    assert_eq!(
        ok("https://rmit.instructure.com/courses/12/modules?x=1"),
        "https://rmit.instructure.com"
    );
    assert_eq!(ok("http://canvas.foo.edu"), "https://canvas.foo.edu");
    assert!(normalise("").is_err());
    assert!(normalise("localhost").is_err());
    assert!(normalise("ftp://canvas.foo.edu").is_err());
}

#[test]
fn a_fresh_data_dir_has_no_profile() {
    let dir = Scratch::new("inst-fresh");
    // The database exists before anyone signs in, so it is no evidence.
    std::fs::write(crate::library::paths::db_path(&dir), b"").unwrap();
    assert_eq!(load(&dir), None);
    assert!(!path(&dir).exists());
}

#[test]
fn an_older_install_is_given_the_unimelb_profile() {
    let dir = Scratch::new("inst-legacy");
    std::fs::write(keyd_core::paths::cookie(&dir), b"canvas_session=x").unwrap();
    assert_eq!(load(&dir), Some(unimelb()));
    assert!(path(&dir).exists(), "the migration is written once");

    let flagged = Scratch::new("inst-legacy-flag");
    std::fs::create_dir_all(keyd_core::paths::session_dir(&flagged)).unwrap();
    std::fs::write(keyd_core::paths::authenticated(&flagged), b"").unwrap();
    assert_eq!(load(&flagged), Some(unimelb()));

    let synced = Scratch::new("inst-legacy-courses");
    std::fs::create_dir_all(synced.join("courses/COMP30026_2026_SM2")).unwrap();
    assert_eq!(load(&synced), Some(unimelb()));
}

#[test]
fn a_saved_profile_round_trips_and_defaults_missing_fields() {
    let dir = Scratch::new("inst-roundtrip");
    std::fs::write(path(&dir), r#"{"canvas":"https://rmit.instructure.com"}"#).unwrap();
    let inst = load(&dir).unwrap();
    assert_eq!(inst.login_path, "/login");
    assert_eq!(inst.idp, Idp::Other);
    assert_eq!(inst.echo360_lead_in_secs, 0);

    save(&dir, &unimelb()).unwrap();
    assert_eq!(load(&dir), Some(unimelb()));
}

#[test]
fn keyd_gets_okta_only_for_an_okta_university() {
    let site = unimelb().site();
    assert_eq!(site, keyd_core::site::Site::unimelb());
    site.check().unwrap();

    let rmit = Institution {
        canvas: "https://rmit.instructure.com".to_string(),
        login_path: "/login".to_string(),
        idp: Idp::Other,
        sso_host: Some("myapps.rmit.edu.au".to_string()),
        echo360_tool: None,
        echo360_lead_in_secs: 0,
        ed_api: Some("https://us.edstem.org/api".to_string()),
        ed_region: Some("us".to_string()),
        exclude_code_prefixes: Vec::new(),
    };
    let site = rmit.site();
    assert_eq!(site.sso, None);
    assert_eq!(site.ed, "https://us.edstem.org");
    site.check().unwrap();
}

#[test]
fn the_hemisphere_comes_from_either_host() {
    assert!(is_southern("https://canvas.lms.unimelb.edu.au", None));
    assert!(is_southern(
        "https://rmit.instructure.com",
        Some("myapps.rmit.edu.au")
    ));
    assert!(!is_southern(
        "https://canvas.harvard.edu",
        Some("key.harvard.edu")
    ));
    assert!(!is_southern("https://foo.instructure.com", None));
}

#[test]
fn the_institution_domain_is_the_universitys_own() {
    let d = institution_domain;
    assert_eq!(
        d("canvas.lms.unimelb.edu.au", Some("sso.unimelb.edu.au")).as_deref(),
        Some("unimelb.edu.au")
    );
    assert_eq!(
        d("canvas.harvard.edu", Some("key.harvard.edu")).as_deref(),
        Some("harvard.edu")
    );
    assert_eq!(
        d("canvas.ox.ac.uk", Some("idp.ox.ac.uk")).as_deref(),
        Some("ox.ac.uk")
    );
    // A shared public suffix is not a domain anyone owns.
    assert_eq!(d("canvas.unimelb.edu.au", Some("sso.monash.edu.au")), None);
    assert_eq!(d("rmit.instructure.com", Some("myapps.rmit.edu.au")), None);
    assert_eq!(d("a.instructure.com", Some("b.instructure.com")), None);
    assert_eq!(d("canvas.foo.edu", None), None);
}
