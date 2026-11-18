//! Old citations after the move: a path written before it (a chat's tool
//! output, an undo snapshot, a saved tab) followed through `path_moves`, and
//! the rules a later rename adds to it (`record_moves`).
//! `app/src/lib/files/pathMoves.ts` is the frontend's copy of the rule.

use std::collections::BTreeMap;

use sqlx::{SqliteConnection, SqlitePool};

use super::rules::{Rule, Rules};

/// Where `rel` went: the longest `path_moves` prefix it starts with, at a
/// path boundary (`/`, `#page=`, `:L15`, `?`), with the rest carried over.
/// `None` when no rule covers it.
pub async fn resolve_moved(pool: &SqlitePool, rel: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar(
        "SELECT new_prefix || substr(?1, length(old_prefix) + 1) FROM path_moves
          WHERE ?1 = old_prefix
             OR (substr(?1, 1, length(old_prefix)) = old_prefix
                 AND substr(?1, length(old_prefix) + 1, 1) IN ('/', '#', ':', '?'))
          ORDER BY length(old_prefix) DESC
          LIMIT 1",
    )
    .bind(rel)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("read path_moves: {e}"))
}

/// What a rename's own rule does to one already kept for its old path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnRule {
    Replace,
    /// The kept rule wins: it names an earlier file old citations meant.
    KeepOlder,
}

/// `path_moves` for renames just made: each rename's own rule, and every
/// path the rules already there send to its old path, straight to its new
/// one, since a lookup takes one step. A path is derived only when the kept
/// rules resolve it to the renamed path itself, so a longer rule (a file
/// renamed on a clash) is never overridden by its folder's. Returns the rows
/// written.
pub async fn record_moves(
    conn: &mut SqliteConnection,
    renames: &[(Rule, OwnRule)],
) -> Result<usize, String> {
    let kept: Vec<Rule> =
        sqlx::query_as::<_, (String, String)>("SELECT old_prefix, new_prefix FROM path_moves")
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| format!("read path_moves: {e}"))?
            .into_iter()
            .map(|(old, new)| Rule { old, new })
            .collect();
    let lookup = Rules::new(&kept);
    let mut chained = BTreeMap::new();
    for (rename, _) in renames {
        for rule in &kept {
            let Some(path) = through(rule, &rename.old) else {
                continue;
            };
            if lookup.map(&path).as_deref() == Some(rename.old.as_str()) {
                chained.insert(path, rename.new.clone());
            }
        }
    }
    let own = renames
        .iter()
        .map(|(rule, own)| (rule.old.clone(), rule.new.clone(), *own));
    let mut written = 0;
    // Derived rules first: a path some rename starts from is that file's.
    let all = chained
        .into_iter()
        .map(|(old, new)| (old, new, OwnRule::Replace))
        .chain(own);
    for (old, new, own) in all {
        let sql = match own {
            OwnRule::Replace => {
                "INSERT OR REPLACE INTO path_moves (old_prefix, new_prefix) VALUES (?1, ?2)"
            }
            OwnRule::KeepOlder => {
                "INSERT OR IGNORE INTO path_moves (old_prefix, new_prefix) VALUES (?1, ?2)"
            }
        };
        written += sqlx::query(sql)
            .bind(&old)
            .bind(&new)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("path_moves {old}: {e}"))?
            .rows_affected() as usize;
    }
    Ok(written)
}

/// The path `rule` sends to `to` when `to` is its new path or under it.
fn through(rule: &Rule, to: &str) -> Option<String> {
    if rule.new == to {
        return Some(rule.old.clone());
    }
    let rest = to.strip_prefix(&rule.new)?;
    rest.starts_with('/').then(|| format!("{}{rest}", rule.old))
}
