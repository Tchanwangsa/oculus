//! A [`PageRegion`] as a GDI region, for `SetWindowRgn`.

use windows_sys::Win32::Graphics::Gdi::{
    CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, HRGN, RGN_COMBINE_MODE, RGN_DIFF,
    RGN_ERROR, RGN_OR,
};

use super::region::{PageRegion, Px, Shape};

/// An owned region, deleted on drop unless handed to the system.
pub(super) struct Rgn(HRGN);

impl Rgn {
    fn rect(r: Px) -> Option<Rgn> {
        // SAFETY: plain GDI object creation; a null handle is a failure.
        let handle = unsafe { CreateRectRgn(r.left, r.top, r.right, r.bottom) };
        (!handle.is_null()).then_some(Rgn(handle))
    }

    /// The circle (or ellipse) inscribed in `r`. GDI leaves a round rect's
    /// right and bottom edge out, hence the extra pixel.
    fn circle(r: Px) -> Option<Rgn> {
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        // SAFETY: as in `rect`.
        let handle = unsafe { CreateRoundRectRgn(r.left, r.top, r.right + 1, r.bottom + 1, w, h) };
        (!handle.is_null()).then_some(Rgn(handle))
    }

    fn combine(&self, other: &Rgn, mode: RGN_COMBINE_MODE) -> Option<()> {
        // SAFETY: both handles are live regions this module owns.
        let kind = unsafe { CombineRgn(self.0, self.0, other.0, mode) };
        (kind != RGN_ERROR).then_some(())
    }

    pub(super) fn handle(&self) -> HRGN {
        self.0
    }

    /// For after `SetWindowRgn` succeeds: the window owns the region now.
    pub(super) fn give_away(self) {
        std::mem::forget(self);
    }
}

impl Drop for Rgn {
    fn drop(&mut self) {
        // SAFETY: the handle is ours and nothing else holds it.
        unsafe { DeleteObject(self.0) };
    }
}

/// A shape's rect with each rounded corner cut away.
fn shape(s: &Shape) -> Option<Rgn> {
    let out = Rgn::rect(s.rect)?;
    for cut in s.cuts() {
        let corner = Rgn::rect(cut.square)?;
        corner.combine(&Rgn::circle(cut.circle)?, RGN_DIFF)?;
        out.combine(&corner, RGN_DIFF)?;
    }
    Some(out)
}

/// Every hole, minus every cover. `None` if GDI fails; an empty region (no
/// holes) is a region, and hides the page.
pub(super) fn build(region: &PageRegion) -> Option<Rgn> {
    let out = Rgn::rect(Px {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    })?;
    for hole in &region.holes {
        out.combine(&shape(hole)?, RGN_OR)?;
    }
    for cover in &region.covers {
        out.combine(&shape(cover)?, RGN_DIFF)?;
    }
    Some(out)
}
