use super::*;
use crate::library::drive::ops::delete::delete_entries_at;
use crate::library::drive::ops::error::Kind;
use crate::library::drive::ops::moves::move_entries_at;
use crate::parse::InFlight;

#[tokio::test]
async fn a_file_being_parsed_or_embedded_is_not_moved_or_deleted() {
    let lib = Lib::new("drive-busy").await;
    let pdf = own(CODE, "Exam/slides.pdf");
    let id = lib.file(&pdf, b"%PDF").await;
    let key = crate::sync::parse_key(&lib.dir, &pdf);

    let claim = InFlight::shared().claim(&key);
    let error = move_entries_at(
        &lib.dir,
        &lib.pool,
        &rels(&[&own(CODE, "Exam")]),
        &root(OTHER),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind, Kind::Io);
    assert!(error.message.contains("being parsed"), "{}", error.message);
    drop(claim);

    let claim = InFlight::embedding().claim(&key);
    let error = delete_entries_at(&lib.dir, &lib.pool, &rels(&[&pdf]))
        .await
        .unwrap_err();
    assert!(
        error.message.contains("being embedded"),
        "{}",
        error.message
    );
    drop(claim);

    assert_eq!(lib.row_of(id).await.unwrap().1, pdf, "nothing moved");
    move_entries_at(&lib.dir, &lib.pool, &rels(&[&pdf]), &root(CODE))
        .await
        .unwrap();
    assert!(
        InFlight::shared().try_claim_all(&[key]).is_ok(),
        "the move let go of its claims"
    );
}
