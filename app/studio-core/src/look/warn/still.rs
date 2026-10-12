//! Dead air: the narration plays while nothing on screen changes, and an
//! empty frame at the opening. A `[pause]` is a hold on purpose, so its
//! silence does not count towards a still span.

use super::super::entities::Entity;
use super::super::limits::{OPENING, STILL};
use super::super::sample::SceneLook;
use super::super::words::while_said;
use super::{Warning, secs, span};
use std::hash::{DefaultHasher, Hash, Hasher};

pub(super) fn check(look: &SceneLook, shown: &[Vec<Entity>], out: &mut Vec<Warning>) {
    let samples = &look.samples;
    let first = shown.iter().position(|e| !e.is_empty());
    let opening = first.map_or(f64::INFINITY, |i| samples[i].t);
    if opening > OPENING {
        let until = opening.min(look.length);
        let message = match first {
            Some(_) => format!(
                "nothing is on screen until {}{}; show something from the start",
                secs(until),
                while_said(&look.words, 0.0, until)
            ),
            None => "nothing is ever on screen".to_owned(),
        };
        out.push(Warning {
            kind: "still",
            noun: "still span",
            short: format!("empty until {}", secs(until)),
            message,
        });
    }
    let Some((lead, end)) = look.narration else {
        return;
    };
    let sig: Vec<u64> = shown.iter().map(|e| signature(e)).collect();
    let mut i = 0;
    while i < samples.len() {
        let mut j = i;
        while j + 1 < samples.len() && sig[j + 1] == sig[i] {
            j += 1;
        }
        let (a, b) = (samples[i].t.max(lead), samples[j].t.min(end));
        let empty_opening = shown[i].is_empty() && first.is_none_or(|f| i < f);
        let paused: f64 = look
            .pauses
            .iter()
            .map(|p| (p.end.min(b) - p.start.max(a)).max(0.0))
            .sum();
        if b - a - paused > STILL && !empty_opening {
            out.push(Warning {
                kind: "still",
                noun: "still span",
                short: span(a, b),
                message: format!(
                    "nothing on screen changes from {:.1} s to {:.1} s{}; give those words something to show",
                    a,
                    b,
                    while_said(&look.words, a, b)
                ),
            });
        }
        i = j + 1;
    }
}

/// What a frame shows: each entity's name, colours, shape, place to the
/// pixel and opacity to 2%.
fn signature(entities: &[Entity]) -> u64 {
    let mut h = DefaultHasher::new();
    for e in entities {
        e.name.hash(&mut h);
        (e.ink, e.form).hash(&mut h);
        let b = e.bbox;
        [b.x0, b.y0, b.x1, b.y1]
            .map(|v| v.round() as i64)
            .hash(&mut h);
        ((e.opacity * 50.0).round() as i64).hash(&mut h);
    }
    h.finish()
}
