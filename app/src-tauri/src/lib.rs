pub mod agents;
mod atomic_write;
mod auth;
mod blocking;
pub mod browser;
pub mod calendar;
pub mod canvas;
pub mod chapters;
pub mod clock;
mod credentials;
pub mod echo360;
pub mod ed;
pub mod embed;
mod files;
pub mod harness;
pub mod keepalive;
pub(crate) mod lecture_jobs;
mod lectures;
pub mod md;
mod media;
pub mod memory;
pub mod menu;
mod migrations;
pub mod mineru;
pub mod okta;
pub mod parse;
pub mod paths;
mod pipeline_events;
pub mod projects;
mod ratelimit;
pub mod reading;
pub mod retrieval;
mod scrape;
mod storage;
pub mod store;
mod subjects;
pub mod sync;
#[cfg(test)]
mod test_support;
pub mod terms;
pub mod voyage;

use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

use auth::{auth_flag_path, saved_session_probe, AuthProbe, AuthState};
use lectures::Echo360Cache;
use scrape::ScrapeCancel;
use subjects::SubjectsState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // ⌘T / ⌘W reach the app as menu events, not key events — see menu.rs.
        .menu(menu::build)
        .on_menu_event(menu::handle)
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .manage(Echo360Cache(Arc::new(Mutex::new(std::collections::HashMap::new()))))
        .manage(lectures::DownloadCancels::default())
        .manage(ScrapeCancel::default())
        .manage(browser::BrowserState::default())
        .setup(|app| {
            // Give the parse and embed seams a window to emit progress to;
            // headless (CLI) runs never bind, so their emits are no-ops.
            parse::events::bind(app.handle().clone());
            embed::events::bind(app.handle().clone());

            // WebKit won't play <video> from the asset protocol (see media.rs).
            app.manage(media::start_media_server(paths::data_dir()));

            echo360::cleanup_partial_downloads(&paths::data_dir());

            // Seeds WebKit with the Canvas session and follows window resizes.
            browser::init(app.handle());

            app.manage(harness::app::init(app.handle()));
            harness::app::reconcile(app.handle());
            // opencode servers left behind by an app that was killed, not quit.
            harness::app::sweep_strays();
            // Clear `running` markers left by chapter/reading runs cut short.
            chapters::app::reconcile(app.handle());
            reading::app::reconcile(app.handle());

            // Session restore: replay the saved cookie with a server-side
            // ping. Rejected → try auto-recover, else sign out; unreachable →
            // stay optimistic, since offline is not expired.
            let app_handle = app.handle().clone();

            if auth_flag_path().exists() {
                eprintln!("[oculus] auth flag found — verifying persisted session");
                let auth_state = app.state::<AuthState>();
                // Optimistic until the async check below corrects it.
                *auth_state.0.lock().unwrap() = true;
                let mem = Arc::clone(&auth_state.0);

                std::thread::spawn(move || match saved_session_probe() {
                    AuthProbe::Valid(_) => {
                        *mem.lock().unwrap() = true;
                        app_handle.emit("canvas-auth-success", "ok").ok();
                    }
                    AuthProbe::Rejected(_) => {
                        // `try_auto_recover` emits its own success event.
                        if okta::try_auto_recover(&app_handle) {
                            *mem.lock().unwrap() = true;
                        } else {
                            eprintln!("[oculus] session rejected — reset to disconnected");
                            std::fs::remove_file(auth_flag_path()).ok();
                            *mem.lock().unwrap() = false;
                            app_handle.emit("canvas-auth-expired", "expired").ok();
                        }
                    }
                    AuthProbe::Unreachable(_) => {
                        eprintln!("[oculus] could not verify session — assuming still good");
                    }
                });
            } else {
                eprintln!("[oculus] no auth flag — fresh session");
            }

            // The LaunchAgent plist holds the CLI's absolute path; re-point it
            // if the bundle moved.
            keepalive::repair_path();

            // Canvas refreshes the session on each request, so a periodic ping
            // holds it open while the app runs (the LaunchAgent covers closed).
            let ka_handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
                if !auth_flag_path().exists() {
                    continue;
                }
                if let AuthProbe::Rejected(_) = saved_session_probe() {
                    if okta::try_auto_recover(&ka_handle) {
                        eprintln!("[oculus] keep-alive: session renewed automatically");
                        continue;
                    }
                    eprintln!("[oculus] keep-alive: session expired");
                    std::fs::remove_file(auth_flag_path()).ok();
                    if let Some(state) = ka_handle.try_state::<AuthState>() {
                        *state.0.lock().unwrap() = false;
                    }
                    ka_handle.emit("canvas-auth-expired", "expired").ok();
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        // Native open panel: returns paths, so file bytes never cross IPC.
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:oculus.db", migrations::all())
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            auth::get_auth_status,
            auth::check_canvas_session,
            auth::launch_canvas_auth,
            auth::disconnect_canvas,
            okta::okta_credential_status,
            okta::okta_save_credentials,
            okta::okta_clear_credentials,
            okta::okta_sign_in,
            keepalive::keepalive_status,
            keepalive::keepalive_enable,
            keepalive::keepalive_disable,
            subjects::sync_subjects,
            subjects::get_subjects,
            scrape::scrape_content,
            scrape::cancel_scrape,
            scrape::rescrape_file,
            scrape::parse_file,
            files::read_course_file,
            files::open_course_file,
            files::scan_parsed_files,
            files::import_uploads,
            files::delete_upload,
            files::create_document,
            files::write_document,
            files::rename_document,
            files::delete_document,
            files::list_documents,
            files::attach_document_image,
            files::attach_document_file,
            calendar::calendar_sync_events,
            lectures::echo360_sync_lectures,
            lectures::echo360_download_video,
            lectures::echo360_cancel_download,
            lectures::echo360_delete_video,
            lectures::echo360_download_transcript,
            lectures::echo360_read_transcript,
            lectures::echo360_clear_transcripts,
            media::media_server_info,
            retrieval::embed_file,
            retrieval::search_pages,
            retrieval::embedding_stats,
            mineru::mineru_set_api_key,
            mineru::mineru_has_api_key,
            mineru::mineru_delete_api_key,
            embed::commands::embed_settings,
            embed::commands::embed_set_engine,
            embed::commands::embed_set_budget,
            embed::commands::embed_blocked,
            embed::commands::embed_estimate,
            parse::commands::parse_settings,
            parse::commands::parse_set_engine,
            parse::commands::parse_set_engine_url,
            parse::commands::parse_probe_local,
            voyage::voyage_set_api_key,
            voyage::voyage_has_api_key,
            voyage::voyage_delete_api_key,
            harness::app::harness_health,
            harness::app::harness_install_offer,
            harness::app::harness_install_run,
            harness::app::harness_sign_in_status,
            harness::app::harness_sign_in_start,
            harness::app::harness_sign_in_code,
            harness::app::harness_sign_in_cancel,
            harness::app::harness_claude_models,
            harness::app::harness_codex_models,
            harness::app::harness_antigravity_models,
            harness::app::harness_antigravity_allow,
            harness::app::harness_antigravity_rules,
            harness::app::harness_antigravity_revoke,
            harness::app::harness_opencode_models,
            harness::app::harness_opencode_providers,
            harness::app::harness_opencode_set_key,
            harness::app::harness_opencode_disconnect,
            harness::app::harness_opencode_oauth_start,
            harness::app::harness_opencode_oauth_finish,
            harness::app::harness_refresh_rate_limits,
            harness::app::harness_send,
            harness::app::harness_edit_resend,
            harness::app::harness_rewind,
            harness::app::harness_queued,
            harness::app::harness_unqueue,
            harness::app::harness_edit_queued,
            harness::app::harness_interrupt,
            harness::app::harness_delete_thread,
            harness::app::document_suggest,
            harness::app::document_suggest_cancel,
            harness::attach::harness_attach_image,
            harness::attach::harness_attach_file,
            chapters::app::lecture_find_chapters,
            chapters::app::lecture_grab_frames,
            reading::app::lecture_write_reading,
            storage::storage_report,
            browser::browser_open_url,
            browser::browser_state,
            browser::browser_place,
            browser::browser_set_viewport,
            browser::browser_hide_tab,
            browser::browser_snapshot,
            browser::browser_hide,
            browser::browser_navigate,
            browser::browser_history,
            browser::browser_reload,
            browser::browser_set_zoom,
            browser::browser_find,
            browser::browser_find_clear,
            browser::browser_close_tab,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Don't let a CLI agent outlive the window.
            if matches!(event, tauri::RunEvent::Exit) {
                harness::app::shutdown(app_handle);
            }
        });
}
