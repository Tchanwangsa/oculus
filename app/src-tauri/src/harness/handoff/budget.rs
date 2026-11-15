//! How much earlier conversation a first prompt may carry. Sizes are UTF-8
//! bytes of JSON, counted as one token each, which keeps the new turn and
//! the imported history pessimistic.

/// The most a handoff may cost, whatever the window allows.
pub(in crate::harness) const DEFAULT_CAP: usize = 16_000;
const BYTE_CAP: usize = 64_000;
/// The window of a model that reported none.
const DEFAULT_WINDOW: u64 = 128_000;

/// What the target session last said of its context: how much is in use,
/// and the window it reported.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::harness) struct Usage {
    pub used: u64,
    pub max: Option<u64>,
}

pub(in crate::harness) struct Budget<'a> {
    pub cap: usize,
    /// The message the handoff rides in front of.
    pub user_text: &'a str,
    /// Attachments sent inside the prompt itself. Oculus sends paths the
    /// agent reads with its own tools, so the app passes none.
    pub images: usize,
    pub files: usize,
    /// The target session's own report; `None` for a session that never ran.
    pub usage: Option<Usage>,
    /// What the session holds when it has reported nothing.
    pub native_estimate: u64,
    /// The target model's window, when the catalogue knows it.
    pub model_window: Option<u64>,
}

/// Without image metadata an image is taken as 8k tokens, another file 4k.
fn attachment_allowance(images: usize, files: usize) -> u64 {
    images as u64 * 8_192 + files as u64 * 4_096
}

pub(in crate::harness) fn json_len(text: &str) -> usize {
    serde_json::to_string(text).map_or(text.len() + 2, |json| json.len())
}

/// The bytes of history that fit: the target's window, less what its
/// session already holds, the new message, and room to work (a quarter of
/// the window, at least 16k), capped. Zero when nothing fits.
pub(in crate::harness) fn handoff_budget(input: &Budget) -> usize {
    let reported = input
        .usage
        .and_then(|usage| usage.max)
        .filter(|max| *max > 0);
    let window = input
        .model_window
        .or(reported)
        .unwrap_or(DEFAULT_WINDOW)
        .min(reported.unwrap_or(u64::MAX));
    let native = input
        .usage
        .map_or(input.native_estimate, |usage| usage.used);
    let current =
        json_len(input.user_text) as u64 + attachment_allowance(input.images, input.files);
    let reserve = 16_000.max(window.div_ceil(4));
    let room = (window as i128) - (native as i128) - (current as i128) - (reserve as i128);
    room.clamp(0, input.cap.min(BYTE_CAP) as i128) as usize
}

/// A session's occupancy survives a change of model; the window it reported
/// under the old one does not, when the new model's is known.
pub(in crate::harness) fn usage_for(
    previous: Option<Usage>,
    same_model: bool,
    known_window: Option<u64>,
) -> Option<Usage> {
    let previous = previous?;
    if same_model {
        return Some(previous);
    }
    let reported = previous.max.filter(|max| *max > 0);
    Some(Usage {
        used: previous.used,
        max: known_window.or(reported),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Budget<'static> {
        Budget {
            cap: 16_000,
            user_text: "Continue",
            images: 0,
            files: 0,
            usage: None,
            native_estimate: 0,
            model_window: Some(32_000),
        }
    }

    /// T3's "subtracts native usage, input, attachments, instructions and
    /// space for work" (`ContextHandoffBudget.test.ts`), case for case.
    #[test]
    fn the_budget_subtracts_native_usage_the_message_attachments_and_room_to_work() {
        let plain = handoff_budget(&base());
        assert!(plain < 16_000);
        let used = Budget {
            native_estimate: 8_000,
            ..base()
        };
        assert!(handoff_budget(&used) < plain);
        let long = "界".repeat(30_000);
        let wordy = Budget {
            user_text: &long,
            ..base()
        };
        assert_eq!(handoff_budget(&wordy), 0);
        let full = Budget {
            usage: Some(Usage {
                used: 23_000,
                max: Some(24_000),
            }),
            ..base()
        };
        assert_eq!(handoff_budget(&full), 0);
        let image = Budget {
            images: 1,
            ..base()
        };
        assert!(handoff_budget(&image) > 4_000 && handoff_budget(&image) < plain);
        let two = Budget {
            images: 2,
            ..base()
        };
        assert_eq!(handoff_budget(&two), 0);
        let big = "x".repeat(70_000);
        let roomy = Budget {
            cap: 64_000,
            model_window: Some(1_000_000),
            user_text: &big,
            images: 1,
            usage: Some(Usage {
                used: 0,
                max: Some(1_000_000),
            }),
            ..base()
        };
        assert_eq!(handoff_budget(&roomy), 64_000);
        let capped = Budget {
            cap: 2_000,
            ..base()
        };
        assert_eq!(handoff_budget(&capped), 2_000);
    }

    /// T3's "keeps measured occupancy across a model change".
    #[test]
    fn occupancy_survives_a_model_change_and_the_old_window_does_not() {
        let previous = Some(Usage {
            used: 37_321,
            max: Some(258_400),
        });
        assert_eq!(usage_for(None, false, None), None);
        assert_eq!(usage_for(previous, true, None), previous);
        assert_eq!(usage_for(previous, false, None), previous);
        let small = Some(Usage {
            used: 30_000,
            max: Some(32_000),
        });
        assert_eq!(
            usage_for(small, false, Some(1_000_000)),
            Some(Usage {
                used: 30_000,
                max: Some(1_000_000)
            })
        );
        let kept = Budget {
            user_text: "Continue work",
            images: 2,
            usage: usage_for(previous, false, None),
            model_window: Some(258_400),
            ..base()
        };
        assert_eq!(handoff_budget(&kept), 16_000);
        // A session that reported nothing is taken at its estimate, against
        // the default window.
        let unknown = Budget {
            user_text: "Continue work",
            native_estimate: 120_000,
            model_window: None,
            ..base()
        };
        assert_eq!(handoff_budget(&unknown), 0);
    }

    /// T3's "reserves context for image batches ... honoring smaller known
    /// windows", over the counts where the default window runs out.
    #[test]
    fn image_batches_take_their_room_and_a_smaller_known_window_wins() {
        let mut previous = 16_000;
        for count in 1..=12 {
            let input = |model_window, usage| Budget {
                user_text: "Compare these screenshots",
                images: count,
                model_window,
                usage,
                ..base()
            };
            let budget = handoff_budget(&input(None, None));
            assert!(budget <= previous, "{count} images");
            previous = budget;
            if count <= 8 {
                assert_eq!(budget, 16_000);
            }
            if count == 10 {
                assert!(budget > 0 && budget < 16_000);
            }
            if count == 12 {
                assert_eq!(budget, 0);
            }
            assert_eq!(handoff_budget(&input(Some(2_000_000), None)), 16_000);
            assert_eq!(handoff_budget(&input(Some(20_000), None)), 0);
            let reported = Some(Usage {
                used: 0,
                max: Some(20_000),
            });
            assert_eq!(handoff_budget(&input(None, reported)), 0);
        }
    }
}
