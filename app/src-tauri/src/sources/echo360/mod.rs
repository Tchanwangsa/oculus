//! Echo360 lecture capture, independent of Tauri. Access is an LTI launch:
//! POSTing Canvas's OAuth-signed tool form to Echo360 mints the session and the
//! CloudFront cookies the media CDN accepts. Echo360's own cookies live in this
//! process only; the Canvas page that carries the form comes through
//! oculus-keyd, which holds the Canvas session.

mod auth;
mod ffmpeg;
mod media;
mod syllabus;

#[cfg(test)]
mod tests;

pub use auth::connect;
pub use ffmpeg::{find_ffmpeg, trim_video};
pub use media::{
    cleanup_partial_downloads, download_url, lecture_dir, partial_path, source_path,
    stream_to_file, transcript, CANCELLED,
};
pub use syllabus::syllabus;

/// Echo360 pads every recording with a fixed lead-in before the lecture starts.
pub const TRIM_SECS: f64 = 14.0;

pub struct Session {
    pub jwt: String,
    play_session: String,
    cf_key_pair_id: String,
    cf_policy: String,
    cf_signature: String,
    cf_tracking: String,
    pub section_id: String,
}

impl Session {
    pub fn cookie_header(&self) -> String {
        format!(
            "ECHO_JWT={}; PLAY_SESSION={}; CloudFront-Key-Pair-Id={}; \
             CloudFront-Policy={}; CloudFront-Signature={}; CloudFront-Tracking2={}",
            self.jwt,
            self.play_session,
            self.cf_key_pair_id,
            self.cf_policy,
            self.cf_signature,
            self.cf_tracking
        )
    }

    pub fn clone_fields(&self) -> Session {
        Session {
            jwt: self.jwt.clone(),
            play_session: self.play_session.clone(),
            cf_key_pair_id: self.cf_key_pair_id.clone(),
            cf_policy: self.cf_policy.clone(),
            cf_signature: self.cf_signature.clone(),
            cf_tracking: self.cf_tracking.clone(),
            section_id: self.section_id.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Lecture {
    pub id: String,
    pub lesson_id: String,
    pub title: String,
    pub date: String,
    pub duration_seconds: i64,
    /// A room-camera stream alongside the Presenter screen (`second_source_hint`).
    pub has_second_source: bool,
}

/// `hd1.mp4` is the Presenter screen, `hd2.mp4` the room camera if any — two
/// files under one media id.
pub type SourceNum = u8;
