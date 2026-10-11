//! The transition into a scene, field by field: the scene's own
//! `transition`, then `video.json`'s `motion.transition`, then the theme's
//! `motion.transition`, then the built-in default (dissolve, 0.7 s,
//! easeInOutCubic, left). A bad value is an error naming the level it came
//! from, e.g. `video.json motion.transition.dur`.

use super::spec::{MotionSpec, TransitionSpec};
use crate::easing::{Easing, NAMED};
use crate::morph::MorphColor;
use crate::suggest::did_you_mean;
use crate::theme::Theme;
use serde_json::Value;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Cut,
    Dissolve,
    Push,
    Wipe,
    Zoom,
    Magic,
}

const LOOKS: &[(&str, Look)] = &[
    ("cut", Look::Cut),
    ("dissolve", Look::Dissolve),
    ("push", Look::Push),
    ("wipe", Look::Wipe),
    ("zoom", Look::Zoom),
    ("magic", Look::Magic),
];

/// Where the incoming scene comes from: `left` pushes the old one out to
/// the left and brings the new one in from the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

const DIRECTIONS: &[(&str, Direction)] = &[
    ("left", Direction::Left),
    ("right", Direction::Right),
    ("up", Direction::Up),
    ("down", Direction::Down),
];

/// The level of the cascade a value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Scene(usize),
    Video,
    Theme,
    Default,
}

impl Level {
    /// `scene 3 transition.kind`, `video.json motion.magicMove`, ...
    fn name(self, field: &str) -> String {
        let prefix = match self {
            Level::Scene(i) => return format!("scene {i} transition.{field}"),
            Level::Video => "video.json motion",
            Level::Theme | Level::Default => "theme motion",
        };
        if matches!(field, "magicMove" | "morphColor" | "motionBlur") {
            format!("{prefix}.{field}")
        } else {
            format!("{prefix}.transition.{field}")
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub look: Look,
    pub dur: f64,
    pub easing: Easing,
    pub direction: Direction,
    /// The scene sets no transition and magicMove is auto: two trees that
    /// share a key morph instead of `look`.
    pub auto_magic: bool,
    /// Where `dur` came from, for the hold check.
    pub dur_level: Level,
    /// How a `magic` transition blends colours.
    pub morph_color: MorphColor,
}

impl Transition {
    /// `p` at `t` seconds into the incoming scene.
    pub fn progress(&self, t: f64) -> f32 {
        self.easing
            .apply((t / self.dur).clamp(0.0, 1.0))
            .clamp(0.0, 1.0) as f32
    }

    /// Whether `t` into the incoming scene still needs both scenes.
    pub fn covers(&self, t: f64) -> bool {
        t < self.dur && (self.look != Look::Cut || self.auto_magic)
    }
}

/// A bad transition value: the level and field it came from, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionError {
    pub level: String,
    pub message: String,
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.level, self.message)
    }
}

const DEFAULT_DUR: f64 = 0.7;

/// The transition into scene `index` (`None` for the first scene, which
/// has nothing to come from).
pub fn resolve(
    index: usize,
    scene: Option<&TransitionSpec>,
    video: &MotionSpec,
    theme: &Theme,
) -> Result<Option<Transition>, TransitionError> {
    if index == 0 {
        return Ok(None);
    }
    let levels = [
        (Level::Scene(index), scene),
        (Level::Video, video.transition.as_ref()),
        (Level::Theme, theme.motion.transition.as_ref()),
    ];
    // The first level that sets the field.
    fn pick<'a, T>(
        levels: &[(Level, Option<&'a TransitionSpec>)],
        field: impl Fn(&'a TransitionSpec) -> Option<&'a T>,
    ) -> Option<(Level, &'a T)> {
        levels
            .iter()
            .find_map(|(l, s)| s.and_then(&field).map(|v| (*l, v)))
    }
    let fail = |level: Level, field: &str, message: String| TransitionError {
        level: level.name(field),
        message,
    };
    let look = match pick(&levels, |s| s.kind.as_ref()) {
        None => Look::Dissolve,
        Some((l, k)) => keyword(k, LOOKS, "transition").map_err(|m| fail(l, "kind", m))?,
    };
    let (dur_level, dur) = match pick(&levels, |s| s.dur.as_ref()) {
        None => (Level::Default, DEFAULT_DUR),
        Some((l, &d)) if d.is_finite() && d > 0.0 => (l, d),
        Some((l, d)) => {
            return Err(fail(
                l,
                "dur",
                format!("{d} is not a positive number of seconds"),
            ));
        }
    };
    let easing = match pick(&levels, |s| s.easing.as_ref()) {
        None => Easing::InOutCubic,
        Some((l, v)) => easing(v, theme).map_err(|m| fail(l, "easing", m))?,
    };
    let direction = match pick(&levels, |s| s.direction.as_ref()) {
        None => Direction::Left,
        Some((l, d)) => keyword(d, DIRECTIONS, "direction").map_err(|m| fail(l, "direction", m))?,
    };
    let magic = [
        (Level::Video, video.magic_move.as_ref()),
        (Level::Theme, theme.motion.magic_move.as_ref()),
    ]
    .into_iter()
    .find_map(|(l, v)| v.map(|v| (l, v)));
    let auto = match magic {
        None => true,
        Some((_, v)) if v == "auto" => true,
        Some((_, v)) if v == "off" => false,
        Some((l, v)) => {
            return Err(fail(
                l,
                "magicMove",
                format!("unknown magicMove `{v}`; use auto or off"),
            ));
        }
    };
    Ok(Some(Transition {
        look,
        dur,
        easing,
        direction,
        auto_magic: auto && scene.is_none(),
        dur_level,
        morph_color: morph_color(scene.map(|s| (index, s)), video, theme)?,
    }))
}

const MORPH_COLORS: &[(&str, MorphColor)] =
    &[("oklch", MorphColor::Oklch), ("oklab", MorphColor::Oklab)];

/// `morphColor`: the scene's `transition`, then `video.json` `motion`, then
/// the theme's `motion`, then oklch. Without a scene, the video's own
/// setting (what an in-scene `<Morph>` uses).
pub fn morph_color(
    scene: Option<(usize, &TransitionSpec)>,
    video: &MotionSpec,
    theme: &Theme,
) -> Result<MorphColor, TransitionError> {
    let levels = [
        scene.and_then(|(i, s)| Some((Level::Scene(i), s.morph_color.as_ref()?))),
        video.morph_color.as_ref().map(|v| (Level::Video, v)),
        theme.motion.morph_color.as_ref().map(|v| (Level::Theme, v)),
    ];
    match levels.into_iter().flatten().next() {
        None => Ok(MorphColor::default()),
        Some((l, v)) => keyword(v, MORPH_COLORS, "morphColor").map_err(|message| TransitionError {
            level: l.name("morphColor"),
            message,
        }),
    }
}

/// Which renders get motion blur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurMode {
    /// The export only.
    Finals,
    Off,
    /// Drafts and previews too.
    Always,
}

const BLUR_MODES: &[(&str, BlurMode)] = &[
    ("finals", BlurMode::Finals),
    ("off", BlurMode::Off),
    ("always", BlurMode::Always),
];

/// `motionBlur`: `video.json` `motion`, then the theme's `motion`, then
/// finals.
pub fn blur_mode(video: &MotionSpec, theme: &Theme) -> Result<BlurMode, TransitionError> {
    let levels = [
        video.motion_blur.as_ref().map(|v| (Level::Video, v)),
        theme.motion.motion_blur.as_ref().map(|v| (Level::Theme, v)),
    ];
    match levels.into_iter().flatten().next() {
        None => Ok(BlurMode::Finals),
        Some((l, v)) => keyword(v, BLUR_MODES, "motionBlur").map_err(|message| TransitionError {
            level: l.name("motionBlur"),
            message,
        }),
    }
}

/// The lint-only checks: the first scene sets no transition, and one that
/// changes the picture fits in its scene's length.
pub fn check_length(
    index: usize,
    scene: Option<&TransitionSpec>,
    resolved: Option<&Transition>,
    length: f64,
) -> Result<(), TransitionError> {
    if index == 0 && scene.is_some() {
        return Err(TransitionError {
            level: "scene 0 transition".to_owned(),
            message: "the first scene has nothing to transition from; remove it".to_owned(),
        });
    }
    if let Some(t) = resolved
        && t.covers(0.0)
        && t.dur > length
    {
        return Err(TransitionError {
            level: t.dur_level.name("dur"),
            message: format!(
                "{} s is longer than scene {index}'s length of {length} s; a transition plays over the start of the incoming scene, so it must fit in it",
                t.dur
            ),
        });
    }
    Ok(())
}

/// A keyword from a list, with a did-you-mean on a miss.
fn keyword<T: Copy>(word: &str, options: &[(&'static str, T)], what: &str) -> Result<T, String> {
    let names = || options.iter().map(|(n, _)| *n);
    options
        .iter()
        .find(|(n, _)| *n == word)
        .map(|(_, v)| *v)
        .ok_or_else(|| {
            format!(
                "unknown {what} `{word}`{}; use {}",
                did_you_mean(word, names()),
                names().collect::<Vec<_>>().join(", ")
            )
        })
}

fn easing(v: &Value, theme: &Theme) -> Result<Easing, String> {
    let names = || {
        let mut n: Vec<&str> = theme.easings.keys().map(String::as_str).collect();
        n.extend(NAMED.iter().map(|(name, _)| *name));
        n
    };
    match v {
        Value::String(s) => {
            if let Some(points) = theme.easings.get(s) {
                return Ok(Easing::Bezier(*points));
            }
            NAMED
                .iter()
                .find(|(n, _)| n == s)
                .map(|(_, e)| *e)
                .ok_or_else(|| {
                    format!(
                        "unknown easing `{s}`{}; use a theme.motion.easing name, a prelude easing or [x1, y1, x2, y2] ({})",
                        did_you_mean(s, names().into_iter()),
                        names().join(", ")
                    )
                })
        }
        Value::Array(a) => {
            let nums: Option<Vec<f64>> = a.iter().map(Value::as_f64).collect();
            match nums.as_deref() {
                Some(&[x1, y1, x2, y2])
                    if [x1, y1, x2, y2].iter().all(|x| x.is_finite())
                        && (0.0..=1.0).contains(&x1)
                        && (0.0..=1.0).contains(&x2) =>
                {
                    Ok(Easing::Bezier([x1, y1, x2, y2]))
                }
                _ => Err(format!(
                    "{} is not cubic-bezier points; use [x1, y1, x2, y2] with x1 and x2 in 0..1",
                    crate::scene::describe(v)
                )),
            }
        }
        other => Err(format!(
            "{} is not an easing; use a theme.motion.easing name, a prelude easing or [x1, y1, x2, y2]",
            crate::scene::describe(other)
        )),
    }
}
