use std::path::Path;

use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

use crate::embed;
use crate::test_support::Scratch;

use super::search::{rank, score_blob};
use super::*;

/// A retired model, as a literal the code under test cannot change.
const RETIRED_MODEL: &str = "Qwen3-VL-Embedding-2B";

/// Just enough schema for the scan; this tests the predicate, not the schema.
async fn fixture(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let db = SqlitePool::connect_with(options).await.unwrap();
    sqlx::query(
        "CREATE TABLE files (
               id INTEGER PRIMARY KEY, subject_id INTEGER, filename TEXT,
               relative_path TEXT, embed_status TEXT, embedded_at TEXT)",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE pages (
               id INTEGER PRIMARY KEY, file_id INTEGER, page_no INTEGER,
               markdown TEXT NOT NULL DEFAULT '', embedding BLOB,
               embed_model TEXT, embed_dim INTEGER, embedded_at TEXT,
               UNIQUE(file_id, page_no))",
    )
    .execute(&db)
    .await
    .unwrap();
    db
}

async fn add_file(db: &SqlitePool, id: i64, subject_id: i64, name: &str) {
    sqlx::query(
        "INSERT INTO files (id, subject_id, filename, relative_path) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(id)
    .bind(subject_id)
    .bind(name)
    .bind(format!("courses/X/{name}"))
    .execute(db)
    .await
    .unwrap();
}

/// A unit vector along one axis, so every score is predictable.
fn axis(index: usize) -> Vec<f32> {
    let mut v = vec![0.0f32; embed::EMBED_DIM];
    v[index] = 1.0;
    v
}

async fn add_page(
    db: &SqlitePool,
    file_id: i64,
    page_no: i64,
    vector: &[f32],
    model: &str,
    dim: i64,
) {
    sqlx::query(
        "INSERT INTO pages (file_id, page_no, markdown, embedding, embed_model, embed_dim)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )
    .bind(file_id)
    .bind(page_no)
    .bind(format!("page {page_no} of {file_id}"))
    .bind(embed::pack_vector(vector).unwrap())
    .bind(model)
    .bind(dim)
    .execute(db)
    .await
    .unwrap();
}

/// A stale vector aimed straight at the query would win if it were scanned.
#[tokio::test]
async fn the_scan_never_sees_another_models_vectors() {
    let scratch = Scratch::new("retrieval-space");
    let db = fixture(&scratch.join("oculus.db")).await;
    add_file(&db, 1, 10, "current.pdf").await;
    add_file(&db, 2, 10, "retired.pdf").await;
    // Current space, a middling match.
    let mut lukewarm = axis(0);
    lukewarm[1] = 1.0;
    add_page(
        &db,
        1,
        1,
        &lukewarm,
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
    )
    .await;
    // Retired space, a perfect match — and it must still lose.
    add_page(&db, 2, 1, &axis(0), RETIRED_MODEL, embed::EMBED_DIM as i64).await;
    db.close().await;

    let hits = rank(
        &scratch.join("oculus.db"),
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
        5,
        &[],
    )
    .await
    .unwrap();
    assert_eq!(hits.len(), 1, "a retired model's vectors were ranked");
    assert_eq!(hits[0].file_id, 1);
}

/// Same model, different `embed_dim`: the pair is the key.
#[tokio::test]
async fn a_truncation_is_a_different_space_too() {
    let scratch = Scratch::new("retrieval-dim");
    let db = fixture(&scratch.join("oculus.db")).await;
    add_file(&db, 1, 10, "narrow.pdf").await;
    add_page(&db, 1, 1, &axis(0), embed::EMBED_MODEL, 256).await;
    db.close().await;

    let hits = rank(
        &scratch.join("oculus.db"),
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
        5,
        &[],
    )
    .await
    .unwrap();
    assert!(hits.is_empty(), "a vector of another width was ranked");
}

#[tokio::test]
async fn the_subject_filter_still_applies_within_the_space() {
    let scratch = Scratch::new("retrieval-subject");
    let db = fixture(&scratch.join("oculus.db")).await;
    add_file(&db, 1, 10, "mine.pdf").await;
    add_file(&db, 2, 20, "theirs.pdf").await;
    add_page(
        &db,
        1,
        1,
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
    )
    .await;
    add_page(
        &db,
        2,
        1,
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
    )
    .await;
    db.close().await;

    let hits = rank(
        &scratch.join("oculus.db"),
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
        5,
        &[20],
    )
    .await
    .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].subject_id, 20);
}

#[tokio::test]
async fn a_current_artifact_ingests_without_a_backend_and_preserves_existing_text() {
    let scratch = Scratch::new("retrieval-cached");
    let path = scratch.join("oculus.db");
    let pdf = scratch.join("cached.pdf");
    std::fs::write(&pdf, b"fixture: no PDF rendering is needed").unwrap();
    let parsed = crate::parse::ParseOutput::new(&pdf, 1, vec![], None, 0);
    std::fs::write(
        crate::parse::pages_path(&pdf),
        serde_json::to_vec(&parsed).unwrap(),
    )
    .unwrap();
    embed::EmbedOutput::new(
        &pdf,
        1,
        vec![embed::EmbedPage {
            page_no: 1,
            vector: embed::encode_vector(&axis(0)).unwrap(),
        }],
    )
    .write(&pdf)
    .unwrap();
    let db = fixture(&path).await;
    add_file(&db, 1, 10, "cached.pdf").await;
    add_page(
        &db,
        1,
        1,
        &axis(1),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
    )
    .await;
    db.close().await;
    let summary = ingest(&path, 1, pdf.to_string_lossy().into_owned(), false)
        .await
        .unwrap();
    assert!(summary.skipped);
    assert_eq!(summary.pages_embedded, 1);
    assert_eq!(summary.pages_with_markdown, 0);
    let hits = rank(
        &path,
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
        1,
        &[],
    )
    .await
    .unwrap();
    assert_eq!(hits[0].score, 1.0);
    assert_eq!(hits[0].markdown, "page 1 of 1");
}

#[test]
fn packed_dot_product_matches_decoding_without_allocating_a_vector() {
    let vector: Vec<_> = (0..embed::EMBED_DIM)
        .map(|i| (i as f32 - 256.0) / 257.0)
        .collect();
    let blob = embed::pack_vector(&vector).unwrap();
    let expected: f32 = embed::unpack_vector(&blob)
        .iter()
        .zip(&vector)
        .map(|(a, b)| a * b)
        .sum();
    assert_eq!(
        score_blob(&blob, &vector).unwrap().to_bits(),
        expected.to_bits()
    );
    assert!(score_blob(&blob[..blob.len() - 2], &vector).is_none());
}

#[tokio::test]
async fn limited_ranking_hydrates_the_best_pages_in_score_order() {
    let scratch = Scratch::new("retrieval-top");
    let path = scratch.join("oculus.db");
    let db = fixture(&path).await;
    add_file(&db, 1, 10, "ranked.pdf").await;
    for page in 1..=80 {
        let mut vector = axis(0);
        vector[1] = (80 - page) as f32;
        add_page(
            &db,
            1,
            page,
            &vector,
            embed::EMBED_MODEL,
            embed::EMBED_DIM as i64,
        )
        .await;
    }
    // Claims the right space, but its blob has the wrong width.
    add_page(
        &db,
        1,
        81,
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
    )
    .await;
    sqlx::query("UPDATE pages SET embedding = ?1 WHERE page_no = 81")
        .bind(vec![0u8; 4])
        .execute(&db)
        .await
        .unwrap();
    db.close().await;
    let hits = rank(
        &path,
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
        3,
        &[],
    )
    .await
    .unwrap();
    assert_eq!(
        hits.iter().map(|hit| hit.page_no).collect::<Vec<_>>(),
        vec![80, 79, 78]
    );
    assert_eq!(hits[0].markdown, "page 80 of 1");
    assert!(hits.windows(2).all(|pair| pair[0].score >= pair[1].score));
}

#[tokio::test]
async fn stats_separate_searchable_from_merely_stored() {
    let scratch = Scratch::new("retrieval-stats");
    let db = fixture(&scratch.join("oculus.db")).await;
    add_file(&db, 1, 10, "old.pdf").await;
    add_file(&db, 2, 10, "new.pdf").await;
    for page in 1..=3 {
        add_page(
            &db,
            1,
            page,
            &axis(0),
            RETIRED_MODEL,
            embed::EMBED_DIM as i64,
        )
        .await;
    }
    add_page(
        &db,
        2,
        1,
        &axis(0),
        embed::EMBED_MODEL,
        embed::EMBED_DIM as i64,
    )
    .await;
    db.close().await;

    let stats = stats(&scratch.join("oculus.db")).await.unwrap();
    assert_eq!(stats.pages_embedded, 1, "stale pages counted as searchable");
    assert_eq!(stats.files_embedded, 1);
    assert_eq!(stats.pages_stored, 4);
    assert_eq!(stats.files_stored, 2);
    assert_eq!(stats.pages_stale, 3);
    assert_eq!(stats.stale_models, vec![RETIRED_MODEL.to_string()]);
    assert_eq!(stats.model.as_deref(), Some(embed::EMBED_MODEL));
    assert_eq!(stats.dim, Some(embed::EMBED_DIM as i64));
}

/// Empty, but still names the space it would search.
#[tokio::test]
async fn an_empty_index_is_zero_everywhere() {
    let scratch = Scratch::new("retrieval-empty");
    fixture(&scratch.join("oculus.db")).await.close().await;
    let stats = stats(&scratch.join("oculus.db")).await.unwrap();
    assert_eq!(stats.pages_embedded, 0);
    assert_eq!(stats.pages_stored, 0);
    assert_eq!(stats.pages_stale, 0);
    assert!(stats.stale_models.is_empty());
    assert_eq!(stats.model.as_deref(), Some(embed::EMBED_MODEL));
}
