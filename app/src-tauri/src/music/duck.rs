//! The duck a focus mode asks for (`music_duck`): lower the volume to a share
//! of what it was and put it back later. The volume before the first duck is
//! kept until the unduck; a volume the user sets meanwhile, here or from the
//! Spotify app, drops it, so their choice is never undone.

/// How far a reported volume may sit from the ducked one and still be the
/// duck's own echo (percent conversions round).
const ECHO_SLACK: u8 = 1;

#[derive(Debug, Default)]
pub(super) struct Duck {
    held: Option<Held>,
}

#[derive(Debug, Clone, Copy)]
struct Held {
    /// The volume before the first duck, restored by the unduck.
    before: u8,
    /// The volume the duck set.
    target: u8,
}

impl Duck {
    /// The volume to set for a duck to `level`% from `current`. A second duck
    /// is a share of the kept volume, not of the ducked one.
    pub(super) fn duck(&mut self, current: u8, level: u8) -> u8 {
        let before = self.held.map_or(current, |h| h.before);
        let target = ((before as u32 * level.min(100) as u32 + 50) / 100) as u8;
        self.held = Some(Held { before, target });
        target
    }

    /// The volume to restore, if a duck is held.
    pub(super) fn unduck(&mut self) -> Option<u8> {
        self.held.take().map(|h| h.before)
    }

    /// A volume the player reported: anything but the duck's own echo is
    /// someone's choice and drops the kept volume.
    pub(super) fn seen(&mut self, volume: u8) {
        if let Some(h) = self.held {
            if volume.abs_diff(h.target) > ECHO_SLACK {
                self.held = None;
            }
        }
    }

    /// The user set the volume here.
    pub(super) fn forget(&mut self) {
        self.held = None;
    }

    #[cfg(test)]
    pub(super) fn is_held(&self) -> bool {
        self.held.is_some()
    }
}
