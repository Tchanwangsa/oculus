// Slide: a real region of a lecture PDF page, made by `oculus studio slide`
// (which prints these props and writes them to assets/slides/<name>.json),
// on a paper card as large as the frame allows. Given the whole page
// (`page`, `pageAspect`, `bbox`), it opens on the page and zooms into the
// crop, the page falling away round it; otherwise the card comes in with an
// Enter preset. Highlights are boxes in fractions of the crop (the sidecar
// lists each block inside it) that draw on at their cue. A caption sits
// beside a narrow crop and below a wide one, the source label under it.
// Keys: `<id>/card`, `<id>/img`, `<id>/page`, `<id>/h<k>`, `<id>/caption`,
// `<id>/source`, so two Slides with one id in a Steps morph from one crop's
// card to the other's. Copy `Slide` into a scene component to compose it.
export const props = {
  // The crop, relative to assets/ (`slides/<name>.png`), and its width / height.
  src: str(),
  aspect: num(),
  // The whole page (`slides/<name>.page.png`), its width / height, and the
  // crop's box on it as fractions [x0, y0, x1, y1]: all three, or no zoom.
  page: str().optional(),
  pageAspect: num().optional(),
  bbox: list(num()).default([]),
  caption: str().optional(),
  // Where the crop is from, e.g. "Lecture 4, p. 12".
  source: str().optional(),
  // Boxes [x0, y0, x1, y1] in fractions of the crop, drawn on from highlightAt[k]
  // (default: one after another once the slide is in).
  highlight: list(list(num())).default([]),
  highlightAt: list(num()).default([]),
  tone: token("color").default("accent"),
  // The slide (or its page) comes in at `at`; the zoom runs from zoomAt for zoomDur.
  at: num().default(0.2),
  zoomAt: num().optional(),
  zoomDur: num().default(1.4),
  enter: oneOf(["rise", "scale", "fade", "blur", "mask"]).default("rise"),
};

export const examples = {
  zoom: {
    src: "starter:slide-figure.png",
    aspect: 1.318,
    page: "starter:slide-page.png",
    pageAspect: 1.7778,
    bbox: [0.4455, 0.1762, 0.9704, 0.8842],
    source: "Lecture 4, p. 12",
    caption: "Each step goes downhill, and the steps shrink as the slope flattens.",
    highlight: [[0.4, 0.66, 0.56, 0.8]],
    highlightAt: [3.4],
  },
  steps: {
    src: "starter:slide-figure.png",
    aspect: 1.318,
    source: "Lecture 4, p. 12",
    highlight: [[0.14, 0.24, 0.24, 0.37], [0.4, 0.66, 0.56, 0.8]],
    highlightAt: [1.6, 2.8],
  },
};

/** A rounded rectangle `w` x `h` inset by `inset`, from its top-left corner clockwise. */
const roundRect = (w, h, r, inset) => {
  const [x0, y0, x1, y1] = [inset, inset, w - inset, h - inset];
  const q = Math.max(0, Math.min(r, (x1 - x0) / 2, (y1 - y0) / 2));
  return (
    `M ${x0 + q} ${y0} H ${x1 - q} A ${q} ${q} 0 0 1 ${x1} ${y0 + q} V ${y1 - q} ` +
    `A ${q} ${q} 0 0 1 ${x1 - q} ${y1} H ${x0 + q} A ${q} ${q} 0 0 1 ${x0} ${y1 - q} ` +
    `V ${y0 + q} A ${q} ${q} 0 0 1 ${x0 + q} ${y0} Z`
  );
};

/** `[x, y, w, h]` of a `aspect` box as large as fits in `w` x `h`, centred there. */
const fit = (x, y, w, h, aspect) => {
  const cw = Math.min(w, h * aspect);
  const ch = cw / aspect;
  return [x + (w - cw) / 2, y + (h - ch) / 2, cw, ch];
};

/**
 * The crop `src` (`aspect` = width / height) on a card in `area`
 * ([x, y, w, h], default the frame less `space.xl` all round), with
 * `caption` and `source`. Options: `page`, `pageAspect`, `bbox` zoom in from
 * the whole page over [zoomAt, zoomAt + zoomDur] (default at + 1); without
 * them `enter` is the card's Enter preset at `at`. `highlight` boxes (crop
 * fractions) draw on at `highlightAt[k]`, in `tone`. `id` prefixes the keys.
 */
const Slide = (
  { src, aspect, page, pageAspect, bbox = [], caption, source, highlight = [], highlightAt = [], tone, at = 0.2, zoomAt, zoomDur = 1.4, enter = "rise", area, id = "slide" },
  { t, theme, size: frame },
) => {
  const { color, space, radius, type, motion } = theme;
  if (!(aspect > 0)) throw new Error(`Slide: aspect must be the crop's width / height, got ${aspect}`);
  const zooms = Boolean(page) && pageAspect > 0 && bbox.length === 4;
  if (page && !zooms) throw new Error("Slide: a page needs pageAspect and bbox [x0, y0, x1, y1] too");
  const ink = tone ?? color.accent;
  const [ax, ay, aw, ah] = area ?? [space.xl, space.xl, frame.width - 2 * space.xl, frame.height - 2 * space.xl];

  // Where the card goes: a caption beside a crop that would leave a third of
  // the width empty, else below it; the source label is one line under all.
  const gap = space.l;
  const label = { ...type.caption, color: color.muted };
  const labelH = source ? label.fontSize * label.lineHeight + space.s : 0;
  const below = { ...type.body, color: color.text };
  const lines = (text, style, w) => Math.max(1, Math.ceil((text.length * style.fontSize * 0.52) / Math.max(w, 1)));
  const belowH = caption ? lines(caption, below, aw) * below.fontSize * below.lineHeight + space.m : 0;
  let card = fit(ax, ay, aw, ah - belowH - labelH, aspect);
  let side = null;
  if (caption && card[2] < aw * 0.66) {
    const colW = Math.min(Math.max((aw - gap) * 0.34, 360), 640);
    card = fit(ax, ay, aw - colW - gap, ah - labelH, aspect);
    // The card and the column sit together, centred in the area.
    const used = card[2] + gap + colW;
    card[0] = ax + (aw - used) / 2;
    side = { x: card[0] + card[2] + gap, w: colW };
  }
  const [cx, cy, cw, ch] = card;

  // The zoom: the card's rect goes from the crop's place on the page (fitted
  // to the frame) to its own, scale by powers and centre linearly, so it reads
  // as one camera move; the page is drawn round the card and fades out.
  const dur = motion.duration.base;
  const ease = (from, over = dur) => easeOutCubic(clamp((t - from) / over, 0, 1));
  const start = zoomAt ?? at + 1;
  const p = zooms ? easeInOutCubic(clamp((t - start) / zoomDur, 0, 1)) : 1;
  let rect = card;
  if (zooms) {
    const [px, py, pw, ph] = fit(0, 0, frame.width, frame.height, pageAspect);
    const [x0, y0, x1, y1] = bbox;
    const from = [px + x0 * pw, py + y0 * ph, (x1 - x0) * pw, (y1 - y0) * ph];
    const w = from[2] * Math.pow(cw / from[2], p);
    const h = from[3] * Math.pow(ch / from[3], p);
    const mx = lerp(from[0] + from[2] / 2, cx + cw / 2, p);
    const my = lerp(from[1] + from[3] / 2, cy + ch / 2, p);
    rect = [mx - w / 2, my - h / 2, w, h];
  }
  const [rx, ry, rw, rh] = rect;
  const shown = zooms ? ease(at) : 1;
  const done = zooms ? start + zoomDur : at + dur;
  // Paper edges grow in as the page falls away, so the crop is seamless at first.
  const paper = p;
  const r = radius.m * paper;

  const pageLayer = () => {
    if (!zooms || p >= 1) return null;
    const [x0, y0, x1, y1] = bbox;
    const pw = rw / (x1 - x0);
    const ph = rh / (y1 - y0);
    return (
      <Image
        key={`${id}/page`}
        src={page}
        style={{ position: "absolute", left: rx - x0 * pw, top: ry - y0 * ph, width: pw, height: ph, objectFit: "fill", opacity: shown * (1 - easeOutCubic(p)), translateY: (1 - shown) * space.m }}
      />
    );
  };

  // A highlight is a tint and a rounded outline that draws on, padded out
  // past the box by the stroke so it frames rather than covers.
  const stroke = Math.max(3, cw * 0.004);
  const cue = (k) => highlightAt[k] ?? done + 0.6 + k * 1.2;
  const mark = (b, k) => {
    if (!Array.isArray(b) || b.length !== 4) throw new Error(`Slide: highlight[${k}] must be [x0, y0, x1, y1] in fractions of the crop`);
    const q = easeInOutCubic(clamp((t - cue(k)) / (dur * 1.6), 0, 1));
    if (q <= 0) return null;
    const pad = stroke * 1.5;
    const w = (b[2] - b[0]) * rw + 2 * pad;
    const h = (b[3] - b[1]) * rh + 2 * pad;
    return (
      <Box key={`${id}/h${k}`} style={{ position: "absolute", left: b[0] * rw - pad, top: b[1] * rh - pad, width: w, height: h }}>
        <Box style={{ position: "absolute", left: 0, top: 0, width: w, height: h, radius: radius.s, fill: ink, opacity: 0.12 * ease(cue(k) + dur * 0.8) }} />
        <Path
          d={roundRect(w, h, radius.s, stroke / 2)}
          viewBox={[0, 0, w, h]}
          stroke={{ width: stroke, color: ink, cap: "round", join: "round" }}
          trim={[0, q]}
          style={{ position: "absolute", left: 0, top: 0, width: w, height: h }}
        />
      </Box>
    );
  };

  const cardStyle = { position: "absolute", left: rx, top: ry, width: rw, height: rh, radius: r, fill: color.surface, opacity: shown };
  if (paper > 0) {
    cardStyle.border = { width: paper, color: color.border };
    cardStyle.shadow = { x: 0, y: space.xs * paper, blur: space.l * paper, spread: 0, color: color.shadow };
  }
  const cardBox = (
    <Box key={`${id}/card`} style={cardStyle}>
      <Image key={`${id}/img`} src={src} style={{ width: rw, height: rh, radius: r, objectFit: "fill" }} />
      {highlight.map(mark)}
    </Box>
  );

  const words = ease(done - dur * 0.5);
  const text = [];
  if (caption) {
    const style = side
      ? { ...type.h2, color: color.text, position: "absolute", left: side.x, top: cy, width: side.w }
      : { ...below, position: "absolute", left: cx, top: cy + ch + space.m, width: cw };
    text.push(
      <Text key={`${id}/caption`} style={{ ...style, opacity: words, translateY: (1 - words) * space.s }}>
        {caption}
      </Text>,
    );
  }
  if (source) {
    const left = side ? side.x : cx;
    const top = side ? cy + ch - label.fontSize * label.lineHeight : cy + ch + belowH + space.s;
    text.push(
      <Row key={`${id}/source`} gap={space.xs} align="center" style={{ position: "absolute", left, top, opacity: words }}>
        <Box style={{ width: space.xs, height: space.xs, radius: radius.pill, fill: ink }} />
        <Text style={label}>{source}</Text>
      </Row>,
    );
  }

  const body = (
    <Box style={{ position: "absolute", left: 0, top: 0, width: frame.width, height: frame.height }}>
      {pageLayer()}
      {cardBox}
      {text}
    </Box>
  );
  return zooms ? body : <Enter preset={enter} at={at}>{body}</Enter>;
};

export default (p, { theme }) => (
  <Box style={{ flexGrow: 1, fill: theme.color.bg }}>
    <Slide {...p} />
  </Box>
);
