use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

use crate::auth::AuthState;
use crate::ipc::IpcPort;
use crate::worker::{ensure_worker_window, WORKER_LABEL};

pub struct SubjectsState(pub Arc<Mutex<Vec<serde_json::Value>>>);

#[tauri::command]
pub fn get_subjects(state: tauri::State<SubjectsState>) -> Vec<serde_json::Value> {
    state.0.lock().unwrap().clone()
}

#[tauri::command]
pub async fn sync_subjects(
    app: AppHandle,
    port: tauri::State<'_, IpcPort>,
    auth: tauri::State<'_, AuthState>,
) -> Result<(), String> {
    let _ = &auth;
    ensure_worker_window(&app, port.0);
    let win = app
        .get_webview_window(WORKER_LABEL)
        .ok_or_else(|| "Worker window unavailable".to_string())?;

    let p = port.0;
    eprintln!("[oculus] evaling sync_subjects, IPC port={p}");

    win.eval(&format!(
        r#"
(async () => {{
    const BASE = `http://127.0.0.1:{p}`;
    console.log('[Oculus] sync_subjects started, IPC port={p}');

    const post = async (path, body) => {{
        const resp = await fetch(BASE + path, {{
            method: 'POST',
            headers: {{ 'Content-Type': 'application/json' }},
            body: typeof body === 'string' ? body : JSON.stringify(body),
        }});
        console.log('[Oculus] POST', path, '->', resp.status);
    }};

    // Canvas fetches go through the Rust cookie proxy (the worker WebView is
    // not logged in — Rust replays the persisted session cookie).
    const cfetch = (canvasPath) =>
        fetch(BASE + '/canvas?url=' + encodeURIComponent('https://canvas.lms.unimelb.edu.au' + canvasPath));

    try {{
        const resp = await cfetch(
            '/api/v1/courses?per_page=100&include[]=term&include[]=account'
        );
        if (!resp.ok) throw new Error(`Canvas API ${{resp.status}}`);
        const all = await resp.json();

        const NON_SUBJECT_PREFIXES = ['MPMP'];
        const academic = all.filter(c =>
            c.term &&
            c.term.name !== 'Default Term' &&
            (c.workflow_state === 'available' || c.workflow_state === 'completed') &&
            !NON_SUBJECT_PREFIXES.some(p => (c.course_code || '').startsWith(p))
        );

        const availableTerms = academic
            .filter(c => c.workflow_state === 'available' && c.term?.name)
            .map(c => c.term.name);
        const latestTerm = [...new Set(availableTerms)].sort().reverse()[0];
        console.log('[Oculus] latest term detected:', latestTerm);

        const courses = academic.map(c => ({{
            ...c,
            _oculus_is_current: c.term?.name === latestTerm && c.workflow_state === 'available',
        }}));

        const nCurrent = courses.filter(c => c._oculus_is_current).length;
        const nPast    = courses.length - nCurrent;
        console.log(`[Oculus] current: ${{nCurrent}} | past: ${{nPast}} | total: ${{courses.length}}`);

        if (courses.length === 0) throw new Error('No courses — session may have expired. Re-authenticate.');

        await post('/subjects', courses);
        console.log('[Oculus] done');
    }} catch (err) {{
        console.error('[Oculus] error:', err);
        try {{ await post('/error', String(err)); }} catch {{}}
    }}
}})();
    "#
    ))
    .map_err(|e| e.to_string())?;

    Ok(())
}
