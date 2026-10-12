//! What a scene shows over time, read from its laid-out trees without
//! painting: sampled every `limits::STEP` seconds and at each cue word
//! (`sample`), flattened into items (`items`, walked as paint composes
//! them in `walk`) and gathered into entities (`entities`). Lint's
//! warnings (`warn`) and `oculus studio beats` (`beats`) read them.

mod beats;
mod entities;
mod items;
mod limits;
mod sample;
mod walk;
mod warn;
mod words;

pub use beats::Beat;
pub use sample::SceneLook;
pub use warn::Warning;

pub(crate) use walk::walk;
