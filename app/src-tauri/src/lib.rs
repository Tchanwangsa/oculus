#[macro_use]
mod elog;

pub mod agents;
pub mod auth;
pub mod db;
pub mod embed;
pub mod harness;
pub mod lectures;
pub mod library;
pub mod music;
pub mod pages;
pub mod parse;
pub mod providers;
pub mod runtime;
pub mod shell;
pub mod sources;
pub mod studio;
pub mod sync;
#[cfg(test)]
mod test_support;
pub mod transcribe;

use std::sync::{Arc, Mutex};
use tauri::Manager;

use auth::AuthState;
use lectures::Echo360Cache;
use sync::scrape::ScrapeCancel;
use sync::subjects::SubjectsState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Before anything opens a file in the data dir (`storage::storage_start_over`).
    library::storage::finish_start_over(&library::paths::data_dir());
    // No credential goes through keyd until it serves the profile's university.
    keyd_core::client::bound::bind(sources::institution::bound);
    tauri::Builder::default()
        // ⌘T / ⌘W reach the app as menu events, not key events — see shell/menu.rs.
        .menu(shell::menu::build)
        .on_menu_event(shell::menu::handle)
        .manage(AuthState(Arc::new(Mutex::new(false))))
        .manage(SubjectsState(Arc::new(Mutex::new(vec![]))))
        .manage(Echo360Cache(Arc::new(Mutex::new(
            std::collections::HashMap::new(),
        ))))
        .manage(lectures::DownloadCancels::default())
        .manage(ScrapeCancel::default())
        .manage(sync::scrape::VideoCancels::default())
        .manage(sync::scrape::embeds::EmbedCancels::default())
        .manage(sync::scrape::embeds::EmbedBearer::default())
        .manage(shell::browser::BrowserState::default())
        .manage(shell::dock::DockState::default())
        .manage(shell::usage::UsageState::default())
        .manage(music::MusicState::default())
        .setup(|app| {
            // WebKit's WebCrypto master key comes from keyd. First, so the
            // default store has its delegate before any page can ask.
            #[cfg(target_os = "macos")]
            shell::webcrypto::install_default(app.handle());
            #[cfg(windows)]
            if let Err(error) = shell::window::fit_startup(app.handle()) {
                elog!("[oculus] could not fit the startup window to the monitor: {error}");
            }

            // Give the parse and embed seams a window to emit progress to;
            // headless (CLI) runs never bind, so their emits are no-ops.
            parse::events::bind(app.handle().clone());
            embed::events::bind(app.handle().clone());

            // WebKit won't play <video> from the asset protocol (see lectures/media.rs).
            app.manage(lectures::media::start_media_server(
                library::paths::data_dir(),
            ));

            sources::echo360::cleanup_partial_downloads(&library::paths::data_dir());

            // How pages share the window with the app: under its transparent
            // webview on macOS, clipped to holes on Windows. Before any page.
            shell::native_layer::init(app.handle());
            // Seeds WebKit with the Canvas session and follows window resizes.
            shell::browser::init(app.handle());
            // Dock pages follow resizes; the frontend creates them (`dock_set_enabled`).
            shell::dock::init(app.handle());
            // A focused browser page gets ⌘-keys before the menu.
            #[cfg(target_os = "macos")]
            shell::keys::install(app.handle());

            app.manage(harness::app::init(app.handle()));
            harness::app::reconcile(app.handle());
            // opencode servers left behind by an app that was killed, not quit.
            harness::app::sweep_strays();
            // Clear `running` markers left by lecture job runs cut short.
            lectures::chapters::app::reconcile(app.handle());
            lectures::lecture_end::app::reconcile(app.handle());
            // Spreadsheets on record without their text (`docs/parsing.md`).
            pages::sheets::reconcile_in_background();

            // Open and active time per hour, from the window and the frontend's pings.
            shell::usage::start(app.handle());

            // The credential broker's LaunchAgent: a release reinstalls its
            // bundled keyd when it changed. Dev builds leave it to the
            // preflight. Before the session is restored, which asks keyd: on
            // the first launch after an update there is none to ask until now.
            let restore = app.handle().clone();
            std::thread::spawn(move || {
                auth::keyd::ensure_installed();
                // keyd sends the session only to the profile's university.
                sources::institution::tell_keyd();
                // Spotify, when Settings → Music is on: its login is keyd's too.
                music::startup::apply_settings(&restore);
                // Replay the stored session with a server-side ping.
                auth::restore_session(&restore);
            });

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
                .add_migrations("sqlite:oculus.db", db::migrations::all())
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            auth::commands::get_auth_status,
            auth::commands::check_canvas_session,
            auth::commands::launch_canvas_auth,
            auth::commands::disconnect_canvas,
            sources::institution::commands::institution_get,
            sources::institution::commands::institution_configure,
            auth::okta::commands::okta_credential_status,
            auth::okta::commands::okta_save_credentials,
            auth::okta::commands::okta_clear_credentials,
            auth::okta::commands::okta_sign_in,
            sync::subjects::sync_subjects,
            sync::subjects::get_subjects,
            sync::scrape::scrape_content,
            sync::scrape::cancel_scrape,
            sync::scrape::videos::canvas_download_video,
            sync::scrape::videos::canvas_cancel_video,
            sync::scrape::embeds::commands::echo360_embed_info,
            sync::scrape::embeds::commands::echo360_embed_download,
            sync::scrape::embeds::commands::echo360_embed_cancel,
            sources::oembed::web_video_info,
            sync::scrape::parse_file,
            sync::scrape::parse_skip,
            library::files::commands::read_course_file,
            library::files::commands::course_file_has_content,
            library::files::commands::open_course_file,
            library::files::commands::scan_parsed_files,
            library::files::uploads::import_uploads,
            library::files::uploads::delete_upload,
            library::files::documents::create_document,
            library::files::documents::write_document,
            library::files::documents::rename_document,
            library::files::documents::delete_document,
            library::files::documents::list_documents,
            library::files::documents::attach_document_image,
            library::files::documents::attach_document_file,
            library::pdf_view::commands::pdf_open,
            library::pdf_view::commands::pdf_render,
            library::pdf_view::commands::pdf_text,
            library::pdf_view::commands::pdf_links,
            library::pdf_view::commands::pdf_close,
            sources::calendar::command::calendar_sync_events,
            lectures::commands::echo360_sync_lectures,
            lectures::commands::echo360_download_video,
            lectures::commands::echo360_cancel_download,
            lectures::commands::echo360_delete_video,
            lectures::commands::echo360_download_transcript,
            lectures::commands::echo360_read_transcript,
            lectures::commands::echo360_clear_transcripts,
            transcribe::app::transcribe_video,
            transcribe::app::apple_speech_status,
            transcribe::app::whisper_models,
            transcribe::app::whisper_download_model,
            transcribe::app::whisper_cancel_download,
            transcribe::app::whisper_delete_model,
            studio::commands::voice::studio_voice_settings,
            studio::commands::voice::studio_set_voice_settings,
            studio::commands::voice::studio_voice_models,
            studio::commands::aligner::studio_aligner_status,
            studio::commands::aligner::studio_aligner_download,
            studio::commands::aligner::studio_aligner_cancel,
            studio::commands::aligner::studio_aligner_delete,
            studio::commands::kokoro::studio_voice_packs_status,
            studio::commands::kokoro::studio_voice_packs_download,
            studio::commands::kokoro::studio_voice_packs_cancel,
            studio::commands::kokoro::studio_voice_packs_delete,
            studio::commands::kokoro::studio_voice_sample,
            studio::commands::voice_files::studio_voices_status,
            studio::commands::voice_files::studio_voice_download,
            studio::commands::voice_files::studio_voice_cancel,
            studio::commands::voice_files::studio_voice_delete,
            studio::commands::pronunciations::studio_pronunciations_list,
            studio::commands::pronunciations::studio_pronunciation_decide,
            studio::commands::pronunciations::studio_pronunciation_wrong,
            studio::commands::pronunciations::studio_pronunciation_sample,
            studio::commands::render::studio_render,
            studio::commands::render::studio_render_cancel,
            studio::commands::render::studio_exports,
            studio::commands::videos::studio_set_captions,
            studio::commands::videos::studio_render_captions,
            studio::commands::videos::studio_videos,
            studio::commands::videos::studio_new_video,
            studio::commands::videos::studio_video_detail,
            studio::commands::videos::studio_frame,
            studio::commands::videos::studio_thumbnail,
            studio::commands::themes::studio_themes,
            studio::commands::themes::studio_theme_source,
            studio::commands::themes::studio_theme_write,
            studio::commands::themes::studio_theme_delete,
            studio::commands::themes::studio_theme_history,
            studio::commands::themes::studio_theme_restore,
            studio::commands::themes::studio_theme_defaults,
            studio::commands::themes::studio_theme_set_default,
            studio::commands::components::studio_components,
            studio::commands::components::studio_component_source,
            studio::commands::components::studio_component_frame,
            studio::commands::components::studio_component_write,
            studio::commands::components::studio_component_history,
            studio::commands::components::studio_component_restore,
            studio::commands::components::studio_component_delete,
            lectures::media::media_server_info,
            pages::retrieval::commands::embed_file,
            pages::retrieval::commands::search_pages,
            pages::retrieval::commands::embedding_stats,
            providers::mineru::mineru_set_api_key,
            providers::mineru::mineru_has_api_key,
            providers::mineru::mineru_delete_api_key,
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
            providers::voyage::voyage_set_api_key,
            providers::voyage::voyage_has_api_key,
            providers::voyage::voyage_delete_api_key,
            providers::groq::groq_set_api_key,
            providers::groq::groq_has_api_key,
            providers::groq::groq_delete_api_key,
            harness::app::setup::harness_health,
            harness::app::setup::harness_install_offer,
            harness::app::setup::harness_install_run,
            harness::app::setup::harness_updates,
            harness::app::setup::harness_update_run,
            harness::app::setup::harness_sign_in_status,
            harness::app::setup::harness_sign_in_start,
            harness::app::setup::harness_sign_in_code,
            harness::app::setup::harness_sign_in_cancel,
            harness::app::models::harness_claude_models,
            harness::app::models::harness_codex_models,
            harness::app::models::harness_antigravity_models,
            harness::app::models::harness_antigravity_allow,
            harness::app::models::harness_antigravity_rules,
            harness::app::models::harness_antigravity_revoke,
            harness::app::models::harness_opencode_models,
            harness::app::models::harness_opencode_models_if_running,
            harness::app::models::harness_opencode_providers,
            harness::app::models::harness_opencode_set_key,
            harness::app::models::harness_opencode_disconnect,
            harness::app::models::harness_opencode_oauth_start,
            harness::app::models::harness_opencode_oauth_finish,
            harness::app::setup::harness_refresh_rate_limits,
            harness::app::turns::harness_send,
            harness::app::turns::harness_edit_resend,
            harness::app::turns::harness_rewind,
            harness::app::turns::harness_queued,
            harness::app::turns::harness_unqueue,
            harness::app::turns::harness_edit_queued,
            harness::app::turns::harness_interrupt,
            harness::app::turns::harness_delete_thread,
            harness::app::suggest::document_suggest,
            harness::app::suggest::document_suggest_cancel,
            harness::attach::harness_attach_image,
            harness::attach::harness_attach_file,
            lectures::chapters::app::lecture_find_chapters,
            lectures::chapters::app::lecture_grab_frames,
            lectures::chapters::app::lecture_thumbnail,
            lectures::lecture_end::app::lecture_find_end,
            library::storage::storage_report,
            library::storage::start_over::storage_start_over,
            shell::usage::usage_activity,
            shell::browser::commands::browser_open_url,
            shell::browser::commands::browser_state,
            shell::browser::commands::browser_place,
            shell::browser::commands::browser_set_viewport,
            shell::browser::commands::browser_hide_tab,
            shell::native_layer::native_set_blocked,
            shell::native_layer::native_set_glass,
            #[cfg(windows)]
            shell::native_layer::windows::native_set_holes,
            shell::browser::commands::browser_hide,
            shell::browser::commands::browser_navigate,
            shell::browser::commands::browser_history,
            shell::browser::commands::browser_reload,
            shell::browser::commands::browser_set_zoom,
            shell::browser::commands::browser_find,
            shell::browser::commands::browser_find_clear,
            shell::browser::commands::browser_close_tab,
            music::commands::music_state,
            music::commands::music_login,
            music::commands::music_connect,
            music::commands::music_disconnect,
            music::commands::music_sign_out,
            music::commands::music_play_pause,
            music::commands::music_play,
            music::commands::music_pause,
            music::commands::music_pause_player,
            music::commands::music_shuffle,
            music::commands::music_repeat,
            music::commands::music_next,
            music::commands::music_prev,
            music::commands::music_seek,
            music::commands::music_volume,
            music::commands::music_play_context,
            music::commands::music_play_track,
            music::commands::music_playlists,
            music::commands::music_playlist_covers,
            music::commands::music_playlist_tracks,
            music::commands::music_album_tracks,
            music::commands::music_tracks,
            music::commands::music_add_to_queue,
            music::commands::music_skip_to,
            music::commands::music_queue_remove,
            music::commands::music_search,
            music::commands::music_search_more,
            music::commands::music_duck,
            music::commands::music_devices,
            music::commands::music_transfer,
            music::commands::music_play_here,
            music::commands::music_outputs,
            music::commands::music_set_output,
            shell::dock::commands::dock_list,
            shell::dock::commands::dock_set_enabled,
            shell::dock::commands::dock_place,
            shell::dock::commands::dock_park,
            shell::dock::commands::dock_set_viewport,
            shell::dock::commands::dock_reload,
            shell::dock::commands::dock_inbox,
            shell::dock::commands::dock_inbox_clear,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Don't let a CLI agent outlive the window, or the Spotify
            // device linger in Spotify's device list.
            if matches!(event, tauri::RunEvent::Exit) {
                harness::app::shutdown(app_handle);
                music::shutdown(app_handle);
            }
        });
}
