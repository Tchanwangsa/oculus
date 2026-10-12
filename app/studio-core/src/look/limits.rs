//! Every threshold the look checks and `beats` use, in frame px and
//! seconds, each with why it is that value.

/// Sample spacing: finer than any beat a viewer notices; a 30 s scene is
/// ~150 layouts.
pub(crate) const STEP: f64 = 0.2;
/// Effective opacity below which a node does not count as shown.
pub(crate) const VISIBLE: f64 = 0.05;
/// A node covering this share of the frame is a background, not content.
pub(crate) const BACKGROUND: f64 = 0.9;

/// How far past an edge counts as off-frame: anti-aliasing and a stroke's
/// half-width may poke out a pixel or two.
pub(crate) const OFF_FRAME_PX: f64 = 4.0;
/// Moving less than this between samples is standing still, so a node
/// sliding in or out through an edge is never off-frame.
pub(crate) const SETTLED_PX: f64 = 1.0;

/// Narration over a frame that does not change for longer than this is
/// dead air (the M5 director flagged 3 s gaps).
pub(crate) const STILL: f64 = 2.5;
/// The first thing should be on screen by now.
pub(crate) const OPENING: f64 = 0.5;

/// Content kept to one half of the frame for longer than this.
pub(crate) const HALF_SPAN: f64 = 3.0;
/// ...while narrower (or, for top and bottom, shorter) than this share of
/// the frame: a clock that fills its half's height measured 45.4%.
pub(crate) const HALF_SIZE: f64 = 0.5;
/// How far past the centre line content may reach and still count as in
/// one half: a clock's labels poke a few px over.
pub(crate) const HALF_SLACK: f64 = 0.03;

/// The end of a scene, where a leftover fragment shows.
pub(crate) const TAIL: f64 = 1.5;
/// A Math showing at most this many pieces...
pub(crate) const FRAGMENT_PIECES: usize = 2;
/// ...and under this share of them is a fragment of a rewrite.
pub(crate) const FRAGMENT_SHARE: f64 = 0.25;
/// A node smaller than this both ways...
pub(crate) const ORPHAN_PX: f64 = 6.0;
/// ...with nothing shown within this distance is a stray mark.
pub(crate) const ORPHAN_GAP: f64 = 40.0;

/// `beats`: a centre moving further than this is a move...
pub(crate) const MOVE_PX: f64 = 8.0;
/// ...and a width or height changing more than this is a resize.
pub(crate) const SIZE_PX: f64 = 8.0;
