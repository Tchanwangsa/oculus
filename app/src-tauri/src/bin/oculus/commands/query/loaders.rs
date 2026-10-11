//! The loaders the read commands and indexing share, over the library tables.

use crate::*;

impl Ctx {
    /// Subject-prefix selection shared by the read commands and indexing.
    pub(crate) fn subject_ids(
        &self,
        pool: &SqlitePool,
        codes: &[String],
    ) -> Result<Vec<i64>, String> {
        subject_ids(&self.rt.block_on(store::subjects(pool))?, codes)
    }

    /// Every library file for the given subjects (all when empty), in the order
    /// `grep` scans and `files` prints: subject, then path.
    pub(super) fn library_files(
        &self,
        pool: &SqlitePool,
        ids: &[i64],
    ) -> Result<Vec<LibFile>, String> {
        let filter = if ids.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            format!(" WHERE f.subject_id IN ({})", list.join(","))
        };
        let sql = format!(
            r#"SELECT f.id AS id, s.code AS code, f.filename AS filename,
                      f.relative_path AS relative_path, f.file_type AS file_type,
                      f.category AS category, f.size_bytes AS size_bytes,
                      f.parse_status AS parse_status,
                      (SELECT COUNT(*) FROM pages p
                        WHERE p.file_id = f.id AND p.markdown != '') AS indexed_pages
               FROM files f JOIN subjects s ON s.id = f.subject_id{filter}
               ORDER BY s.code, f.relative_path"#
        );
        self.rt.block_on(async {
            let rows = sqlx::query(&sql)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            Ok(rows
                .iter()
                .map(|r| LibFile {
                    id: r.try_get("id").unwrap_or_default(),
                    code: r.try_get("code").unwrap_or_default(),
                    filename: r.try_get("filename").unwrap_or_default(),
                    relative_path: r.try_get("relative_path").unwrap_or_default(),
                    file_type: r.try_get("file_type").unwrap_or_default(),
                    category: r.try_get("category").ok().flatten(),
                    size_bytes: r.try_get("size_bytes").ok().flatten(),
                    parse_status: r.try_get("parse_status").ok().flatten(),
                    indexed_pages: r.try_get("indexed_pages").unwrap_or_default(),
                })
                .collect())
        })
    }

    /// One file's parsed pages, in order.
    pub(super) fn pages_of(
        &self,
        pool: &SqlitePool,
        file_id: i64,
    ) -> Result<Vec<(i64, String)>, String> {
        self.rt.block_on(async {
            let rows = sqlx::query(
                "SELECT page_no, markdown FROM pages
                  WHERE file_id = ?1 AND markdown != '' ORDER BY page_no",
            )
            .bind(file_id)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            Ok(rows
                .iter()
                .map(|r| {
                    (
                        r.try_get("page_no").unwrap_or_default(),
                        r.try_get("markdown").unwrap_or_default(),
                    )
                })
                .collect())
        })
    }

    /// Parsed page markdown, keyed by file and ordered by page number.
    ///
    /// Loaded whole in one query rather than one query per file.
    pub(super) fn page_text(
        &self,
        pool: &SqlitePool,
        ids: &[i64],
    ) -> Result<HashMap<i64, Vec<(i64, String)>>, String> {
        let filter = if ids.is_empty() {
            String::new()
        } else {
            let list: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
            format!(" AND f.subject_id IN ({})", list.join(","))
        };
        let sql = format!(
            r#"SELECT p.file_id AS file_id, p.page_no AS page_no, p.markdown AS markdown
               FROM pages p JOIN files f ON f.id = p.file_id
               WHERE p.markdown != ''{filter}
               ORDER BY p.file_id, p.page_no"#
        );
        self.rt.block_on(async {
            let rows = sqlx::query(&sql)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            let mut out: HashMap<i64, Vec<(i64, String)>> = HashMap::new();
            for r in &rows {
                let id: i64 = r.try_get("file_id").unwrap_or_default();
                let page: i64 = r.try_get("page_no").unwrap_or_default();
                let md: String = r.try_get("markdown").unwrap_or_default();
                out.entry(id).or_default().push((page, md));
            }
            Ok(out)
        })
    }
}
