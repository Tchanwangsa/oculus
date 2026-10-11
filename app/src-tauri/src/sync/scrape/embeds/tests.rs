use super::naming::video_rel;
use super::row::{current, EmbedRow};
use crate::db::store::{echo_embed, upsert_echo_embed, EchoEmbed};

const MEDIA: &str = "943d56e6-1111-2222-3333-444444444444";

#[test]
fn a_video_takes_its_media_name_as_an_mp4() {
    let none = |_: &str| false;
    assert_eq!(
        video_rel("THTR30042", "WWW_WOTWHYWHO_2024.mp4", MEDIA, &none).unwrap(),
        "courses/THTR30042/files/WWW_WOTWHYWHO_2024.mp4"
    );
    assert_eq!(
        video_rel("THTR30042", "Day One.MOV", MEDIA, &none).unwrap(),
        "courses/THTR30042/files/Day_One.mp4"
    );
    assert_eq!(
        video_rel("THTR30042", "notes.v2", MEDIA, &none).unwrap(),
        "courses/THTR30042/files/notes.v2.mp4"
    );
    assert_eq!(
        video_rel("THTR30042", "  ", MEDIA, &none).unwrap(),
        "courses/THTR30042/files/echo-943d56e6.mp4"
    );
}

#[test]
fn a_taken_name_gets_the_media_id_prefix() {
    let plain = "courses/C/files/Lecture.mp4";
    let suffixed = "courses/C/files/Lecture-943d56e6.mp4";
    assert_eq!(
        video_rel("C", "Lecture.mp4", MEDIA, &|rel| rel == plain).unwrap(),
        suffixed
    );
    // Both taken: the suffixed one names this media, so it is replaced.
    assert_eq!(
        video_rel("C", "Lecture.mp4", MEDIA, &|rel| rel == plain
            || rel == suffixed)
        .unwrap(),
        suffixed
    );
}

#[test]
fn the_row_reaches_the_frontend_in_camel_case() {
    let row = EmbedRow {
        link_id: "l".into(),
        media_id: "m".into(),
        name: "n.mp4".into(),
        duration_s: Some(1.5),
        restrict_download: false,
        has_captions: true,
        poster_rel: Some("courses/C/images/echo-m.jpg".into()),
        file_rel: None,
        extra_sources: 0,
    };
    assert_eq!(
        serde_json::to_value(&row).unwrap(),
        serde_json::json!({
            "linkId": "l", "mediaId": "m", "name": "n.mp4", "durationS": 1.5,
            "restrictDownload": false, "hasCaptions": true,
            "posterRel": "courses/C/images/echo-m.jpg", "fileRel": null, "extraSources": 0
        })
    );
}

#[tokio::test]
async fn a_deleted_download_is_forgotten() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'C', 'Course')")
        .execute(&pool)
        .await
        .unwrap();
    let dir = crate::test_support::Scratch::new("echo-embed-row");
    std::fs::create_dir_all(dir.join("courses/C/images")).unwrap();
    std::fs::write(dir.join("courses/C/images/echo-m.jpg"), b"jpg").unwrap();
    let embed = EchoEmbed {
        link_id: "l".into(),
        subject_id: 1,
        media_id: "m".into(),
        secure_link_id: "s".into(),
        name: "n.mp4".into(),
        duration_s: None,
        restrict_download: false,
        has_captions: false,
        poster_rel: Some("courses/C/images/echo-m.jpg".into()),
        file_rel: None,
        extra_sources: 0,
    };
    upsert_echo_embed(&pool, &embed).await.unwrap();
    crate::db::store::set_echo_embed_file(&pool, "l", Some("courses/C/files/n.mp4"))
        .await
        .unwrap();

    let stored = echo_embed(&pool, "l").await.unwrap().unwrap();
    let row = current(&pool, &dir, stored).await.unwrap();
    assert_eq!(row.file_rel, None);
    assert_eq!(
        row.poster_rel.as_deref(),
        Some("courses/C/images/echo-m.jpg")
    );
    let after = echo_embed(&pool, "l").await.unwrap().unwrap();
    assert_eq!(after.file_rel, None, "the row is cleared too");
}
