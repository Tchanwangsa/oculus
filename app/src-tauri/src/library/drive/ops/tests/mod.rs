//! A scratch library with a migrated database: two subjects, files placed
//! with their rows, and a parsed PDF with every artifact.

mod busy;
mod copy;
mod create;
mod delete;
mod imports;
mod moves;
mod names;
mod pictures;
mod rename;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

use crate::library::layout::{course_rel, derived_rel, synced, DerivedKind, SyncedKind};
use crate::test_support::Scratch;

pub const CODE: &str = "COMP30026_2026_SM2";
pub const OTHER: &str = "INFO30006_2026_SM2";

pub struct Lib {
    pub dir: Scratch,
    pub pool: SqlitePool,
}

impl Lib {
    pub async fn new(name: &str) -> Self {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for migration in crate::db::migrations::all() {
            sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
        }
        for (id, code) in [(1, CODE), (2, OTHER)] {
            sqlx::query("INSERT INTO subjects (id, code, name) VALUES (?1, ?2, ?2)")
                .bind(id)
                .bind(code)
                .execute(&pool)
                .await
                .unwrap();
        }
        let lib = Self {
            dir: Scratch::new(name),
            pool,
        };
        for code in [CODE, OTHER] {
            lib.mkdir(&course_rel(code));
        }
        lib
    }

    pub fn path(&self, rel: &str) -> std::path::PathBuf {
        self.dir.join(rel)
    }

    pub fn put(&self, rel: &str, bytes: &[u8]) {
        crate::test_support::write_creating(self.path(rel), bytes);
    }

    pub fn mkdir(&self, rel: &str) {
        std::fs::create_dir_all(self.path(rel)).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap()
    }

    pub fn exists(&self, rel: &str) -> bool {
        std::fs::symlink_metadata(self.path(rel)).is_ok()
    }

    /// A file on disk with its row; returns the row's id.
    pub async fn file(&self, rel: &str, bytes: &[u8]) -> i64 {
        self.put(rel, bytes);
        self.row(rel).await
    }

    pub async fn row(&self, rel: &str) -> i64 {
        let subject = if rel.contains(OTHER) { 2 } else { 1 };
        let name = rel.rsplit('/').next().unwrap();
        let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        sqlx::query(
            "INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category)
             VALUES (?1, ?2, ?3, ?4, 1, ?5)",
        )
        .bind(subject)
        .bind(name)
        .bind(rel)
        .bind(ext)
        .bind(crate::library::layout::category_of(
            crate::library::layout::within_course(rel),
        ))
        .execute(&self.pool)
        .await
        .unwrap()
        .last_insert_rowid()
    }

    /// `(subject_id, relative_path, filename, file_type, category)` of a row.
    pub async fn row_of(&self, id: i64) -> Option<(i64, String, String, String, String)> {
        sqlx::query(
            "SELECT subject_id, relative_path, filename, file_type, category FROM files WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .unwrap()
        .map(|r| {
            (
                r.get(0),
                r.get(1),
                r.get(2),
                r.get(3),
                r.get(4),
            )
        })
    }

    pub async fn pages_of(&self, id: i64) -> Vec<String> {
        sqlx::query_scalar("SELECT markdown FROM pages WHERE file_id = ?1 ORDER BY page_no")
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }

    /// A parsed and embedded PDF at `rel`: the file, its row, its text and
    /// pictures beside it, its records in the derived folder, and one page
    /// row whose markdown links a picture.
    pub async fn parsed_pdf(&self, rel: &str) -> i64 {
        let id = self.file(rel, b"%PDF-1.4").await;
        let prefix = images_prefix(rel);
        let page = format!("Intro ![]({prefix}/x.png) end");
        self.put(&derived_rel(rel, DerivedKind::Markdown), page.as_bytes());
        self.put(
            &format!("{}/x.png", derived_rel(rel, DerivedKind::Images)),
            b"png",
        );
        let record = serde_json::json!({
            "pdf": rel.rsplit('/').next().unwrap(),
            "mode": crate::parse::MODE,
            "parser_version": 3,
            "page_count": 1,
            "pages": [{
                "page_no": 1,
                "markdown": page,
                "blocks": [
                    {"kind": "text", "bbox": [0.0, 0.0, 1.0, 0.1], "start": 0, "end": 5},
                    {"kind": "image", "bbox": [0.0, 0.2, 1.0, 0.8], "start": 6, "end": 6 + 4 + prefix.len() + 7},
                    {"kind": "text", "bbox": [0.0, 0.9, 1.0, 1.0], "start": page.len() - 3, "end": page.len()}
                ]
            }]
        });
        self.put(
            &derived_rel(rel, DerivedKind::Pages),
            &serde_json::to_vec(&record).unwrap(),
        );
        self.put(
            &derived_rel(rel, DerivedKind::Embeddings),
            b"{\"pdf\":\"x\"}",
        );
        sqlx::query("INSERT INTO pages (file_id, page_no, markdown) VALUES (?1, 1, ?2)")
            .bind(id)
            .bind(&page)
            .execute(&self.pool)
            .await
            .unwrap();
        id
    }
}

pub fn images_prefix(rel: &str) -> String {
    derived_rel(rel, DerivedKind::Images)
        .rsplit('/')
        .next()
        .unwrap()
        .to_string()
}

/// `courses/<code>/<inner>`.
pub fn own(code: &str, inner: &str) -> String {
    format!("{}/{inner}", course_rel(code))
}

/// A synced file of `CODE`'s.
pub fn course_file(name: &str) -> String {
    crate::library::layout::in_course(CODE, &synced(SyncedKind::Files, name))
}

pub fn root(code: &str) -> String {
    course_rel(code)
}

pub fn rels(paths: &[&str]) -> Vec<String> {
    paths.iter().map(|p| p.to_string()).collect()
}
