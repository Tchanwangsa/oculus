//! Scene transitions: how one scene gives way to the next. A transition
//! plays over the first `dur` seconds of the incoming scene B, with the
//! outgoing scene A held at its last moment; the video's length stays the
//! sum of holds. Each frame lays out both scenes and paints them onto one
//! canvas (`composite`), or morphs A's tree into B's (`magic`).

mod cascade;
mod composite;
mod spec;

pub use cascade::{
    BlurMode, Look, Transition, TransitionError, blur_mode, check_hold, morph_color, resolve,
};
pub use composite::layers;
pub use spec::{MotionSpec, TransitionSpec};

#[cfg(test)]
mod tests;
