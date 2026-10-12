//! Echo360 videos embedded in Canvas pages, one LTI launch per link, apart
//! from the course-section flow in `auth.rs`. The protocol and its traps are
//! in `docs/auth.md`; nothing here knows about Tauri or the database.

mod bearer;
mod cdn;
mod download;
mod files;
mod launch;
mod link;
mod playlist;
mod properties;
mod region;
mod sign;

#[cfg(test)]
mod tests;

pub use bearer::{redeem, Bearer};
pub use cdn::get_bytes;
pub use download::{captions_path, download, Job};
pub use files::{media_files, MediaFiles};
pub use launch::{agent, launch, Launch};
pub use link::{is_safe_id, region_of, retrieve_url};
pub use playlist::UNSUPPORTED;
pub use properties::{fetch as properties, EmbedInfo, PropertiesError, QueryString};
pub use region::in_region;
pub use sign::sign;
