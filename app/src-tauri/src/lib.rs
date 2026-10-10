pub mod agents;
mod atomic_write;
pub mod auth;
mod blocking;
pub mod browser;
mod bundled;
pub mod calendar;
pub mod canvas;
pub mod chapters;
pub mod clock;
mod credentials;
pub mod echo360;
pub mod ed;
pub mod embed;
mod files;
pub mod groq;
pub mod harness;
pub mod keyd;
#[cfg(target_os = "macos")]
mod keys;
pub mod lecture_end;
pub(crate) mod lecture_jobs;
mod lectures;
mod legacy_agent;
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
pub mod retrieval;
mod scrape;
pub mod sheets;
mod storage;
pub mod store;
mod subjects;
pub mod sync;
pub mod terms;
#[cfg(test)]
mod test_support;
pub mod transcribe;
mod usage;
pub mod voyage;

use std::sync::{Arc, Mutex};
use tauri::Manager;

use auth::AuthState;
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
        .manage(Echo360Cache(Arc::new(Mutex::new(
            std::collections::HashMap::new(),
        ))))
        .manage(lectures::DownloadCancels::default())
        .manage(ScrapeCancel::default())
        .manage(scrape::VideoCancels::default())
        .manage(browser::BrowserState::default())
        .manage(usage::UsageState::default())
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
            // A focused browser page gets ⌘-keys before the menu.
            #[cfg(target_os = "macos")]
            keys::install(app.handle());

            app.manage(harness::app::init(app.handle()));
            harness::app::reconcile(app.handle());
            // opencode servers left behind by an app that was killed, not quit.
            harness::app::sweep_strays();
            // Clear `running` markers left by lecture job runs cut short.
            chapters::app::reconcile(app.handle());
            lecture_end::app::reconcile(app.handle());
            // Spreadsheets on record without their text (`docs/parsing.md`).
            sheets::reconcile_in_background();

            // Open and active time per hour, from the window and the frontend's pings.
            usage::start(app.handle());

            // Session restore: replay the stored session with a server-side ping.
            auth::restore_session(app.handle());

            // The session keep-alive agent an earlier version installed.
            legacy_agent::retire_in_background();

            // The credential broker's LaunchAgent; a release reinstalls its
            // bundled keyd when it changed. Dev builds leave it to the preflight.
            keyd::ensure_installed();

            Ok(())
        })
        // No click handler: it would be injected into browser pages too,
        // cancel their `target=_blank` links and call IPC they cannot reach.
        // The app's own links go through `AppLayout`'s capture-phase handler.
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        // Native open panel: returns paths, so file bytes never cross IPC.
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:oculus.db", migrations::all())
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            auth::commands::get_auth_status,
            auth::commands::check_canvas_session,
            auth::commands::launch_canvas_auth,
            auth::commands::disconnect_canvas,
            okta::commands::okta_credential_status,
            okta::commands::okta_save_credentials,
            okta::commands::okta_clear_credentials,
            okta::commands::okta_sign_in,
            subjects::sync_subjects,
            subjects::get_subjects,
            scrape::scrape_content,
            scrape::cancel_scrape,
            scrape::canvas_download_video,
            scrape::canvas_cancel_video,
            scrape::parse_file,
            scrape::parse_skip,
            files::read_course_file,
            files::course_file_has_content,
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
            transcribe::app::transcribe_video,
            transcribe::app::apple_speech_status,
            transcribe::app::whisper_models,
            transcribe::app::whisper_download_model,
            transcribe::app::whisper_cancel_download,
            transcribe::app::whisper_delete_model,
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
            parse::commands::parse_set_accept_expired_result_cert,
            parse::commands::parse_result_cert,
            parse::commands::parse_probe_local,
            voyage::voyage_set_api_key,
            voyage::voyage_has_api_key,
            voyage::voyage_delete_api_key,
            groq::groq_set_api_key,
            groq::groq_has_api_key,
            groq::groq_delete_api_key,
            harness::app::harness_health,
            harness::app::harness_install_offer,
            harness::app::harness_install_run,
            harness::app::harness_updates,
            harness::app::harness_update_run,
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
            chapters::app::lecture_thumbnail,
            lecture_end::app::lecture_find_end,
            storage::storage_report,
            usage::usage_activity,
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
