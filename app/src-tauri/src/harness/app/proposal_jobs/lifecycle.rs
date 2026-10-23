use super::runner::run;
use super::validate;
use crate::harness::app::publish_proposal;
use serde_json::json;
use sqlx::SqlitePool;
use tauri::AppHandle;
#[tauri::command]
pub async fn proposal_job_start(
    app: AppHandle,
    id: String,
) -> Result<crate::db::proposals::Proposal, String> {
    let pool = crate::db::store::open_pool().await?;
    let (proposal, claimed) = claim(&pool, &id).await?;
    if !claimed {
        return Ok(proposal);
    }
    if let Err(error) = publish_proposal(&app, proposal.clone()).await {
        elog!("[oculus] proposal job event: {error}");
    }
    let running = proposal.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = run(&app, &running.kind, &running.state).await;
        if let Err(e) = finish(&app, &pool, &id, outcome).await {
            elog!("[oculus] proposal job: {e}");
        }
    });
    Ok(proposal)
}

async fn finish(
    app: &AppHandle,
    pool: &SqlitePool,
    id: &str,
    outcome: Result<(), String>,
) -> Result<(), String> {
    let proposal = crate::db::proposals::get(pool, id).await?;
    let mut state = proposal.state;
    state["job"] = match outcome {
        Ok(()) => json!({"phase":"complete"}),
        Err(error) => json!({"phase":"error","error":error}),
    };
    sqlx::query("UPDATE chat_proposals SET state=?3, told=NULL, updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE item_id=?1 AND block=?2 AND status='created'")
        .bind(proposal.item_id).bind(proposal.block).bind(state.to_string()).execute(pool).await.map_err(|e|e.to_string())?;
    publish_proposal(app, crate::db::proposals::get(pool, id).await?).await
}

/// A process restart cannot resume an external job. Keep its confirmation
/// decided and surface the interruption instead of rearming a billed action.
pub(crate) async fn reconcile(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("UPDATE chat_proposals SET told=NULL, state=json_set(state,'$.job',json_object('phase','error','error','The app closed before this job finished; check its result before proposing another run.')) WHERE status='created' AND json_extract(state,'$.job.phase')='running'")
        .execute(pool).await.map(|_|()).map_err(|e|e.to_string())
}

async fn claim(
    pool: &SqlitePool,
    id: &str,
) -> Result<(crate::db::proposals::Proposal, bool), String> {
    let proposal = crate::db::proposals::get(pool, id).await?;
    validate(&proposal.kind, &proposal.state)?;
    let mut state = proposal.state.clone();
    state["job"] = json!({"phase":"running"});
    let changed = sqlx::query("UPDATE chat_proposals SET status = 'created', state = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE item_id = ?1 AND block = ?2 AND status = 'pending' AND state = ?4 AND EXISTS (SELECT 1 FROM harness_items i JOIN harness_threads t ON t.id=i.thread_id WHERE i.id=chat_proposals.item_id AND i.thread_id=chat_proposals.thread_id)")
        .bind(proposal.item_id).bind(proposal.block).bind(state.to_string()).bind(proposal.state.to_string()).execute(pool).await.map_err(|e| e.to_string())?.rows_affected();
    Ok((crate::db::proposals::get(pool, id).await?, changed > 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn a_job_is_claimed_once_and_restart_cannot_rearm_it() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../../../db/proposals/schema.sql"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TABLE harness_threads(id INTEGER);CREATE TABLE harness_items(id INTEGER,thread_id INTEGER);INSERT INTO harness_threads VALUES(1);INSERT INTO harness_items VALUES(12,1);").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chat_proposals(item_id,block,thread_id,kind,state,initial_state) VALUES(12,0,1,'Download','{\"item\":\"aligner\"}','{\"item\":\"aligner\"}')").execute(&pool).await.unwrap();
        let (a, b) = tokio::join!(claim(&pool, "12.0"), claim(&pool, "12.0"));
        assert_ne!(a.unwrap().1, b.unwrap().1);
        let proposal = crate::db::proposals::get(&pool, "12.0").await.unwrap();
        assert_eq!(proposal.status, "created");
        assert_eq!(proposal.state["job"]["phase"], "running");
        reconcile(&pool).await.unwrap();
        let (proposal, again) = claim(&pool, "12.0").await.unwrap();
        assert!(!again);
        assert_eq!(proposal.state["job"]["phase"], "error");
        assert_eq!(proposal.told, None);
        sqlx::raw_sql("UPDATE chat_proposals SET status='pending';DELETE FROM harness_items")
            .execute(&pool)
            .await
            .unwrap();
        assert!(!claim(&pool, "12.0").await.unwrap().1);
    }
}
