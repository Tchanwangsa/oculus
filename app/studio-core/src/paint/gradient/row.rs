//! One row of a ramp. Along a row the offset is linear in x and the colour
//! piecewise linear in the offset, so each stretch between two stops is
//! filled by stepping its colour, plus the dither threshold, truncated.

/// Premultiplied stops, colour channels 0 to 255, offsets rising.
pub type Stops = [(f64, [f32; 4])];

/// Fills `row` (premultiplied RGBA8) at offsets `s0 + ds·i`; `bayer` is
/// the row's thresholds and `phase` the first pixel's column in them.
pub fn fill(row: &mut [u8], stops: &Stops, s0: f64, ds: f64, bayer: &[f32; 8], phase: usize) {
    let w = row.len() / 4;
    let n = stops.len();
    let mut i = 0;
    while i < w {
        let s = s0 + ds * i as f64;
        // Stops at or before s: 0 and n are the pads past each end.
        let k = stops.iter().take_while(|(o, _)| *o <= s).count();
        let (c, dc, edge) = match k {
            0 => (stops[0].1, [0.0; 4], (ds > 0.0).then_some(stops[0].0)),
            k if k == n => (
                stops[n - 1].1,
                [0.0; 4],
                (ds < 0.0).then_some(stops[n - 1].0),
            ),
            k => {
                let ((oa, ca), (ob, cb)) = (stops[k - 1], stops[k]);
                let per = ((s - oa) / (ob - oa)) as f32;
                let step = (ds / (ob - oa)) as f32;
                let c = [0, 1, 2, 3].map(|ch| ca[ch] + (cb[ch] - ca[ch]) * per);
                let dc = [0, 1, 2, 3].map(|ch| (cb[ch] - ca[ch]) * step);
                (c, dc, if ds > 0.0 { Some(ob) } else { Some(oa) })
            }
        };
        // The first pixel past this stretch; continuity makes an off-by-one
        // here harmless.
        let end = match edge {
            Some(o) if ds != 0.0 => {
                let x = (o - s0) / ds;
                let x = if ds > 0.0 { x.ceil() } else { x.floor() + 1.0 };
                (x.max(0.0) as usize).clamp(i + 1, w)
            }
            _ => w,
        };
        for (m, px) in row[i * 4..end * 4]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .enumerate()
        {
            let d = bayer[(phase + i + m) & 7];
            let t = m as f32;
            // `as u8` truncates and saturates: floor(v + d), clamped.
            let a = (c[3] + dc[3] * t + d) as u8;
            px[0] = ((c[0] + dc[0] * t + d) as u8).min(a);
            px[1] = ((c[1] + dc[1] * t + d) as u8).min(a);
            px[2] = ((c[2] + dc[2] * t + d) as u8).min(a);
            px[3] = a;
        }
        i = end;
    }
}
