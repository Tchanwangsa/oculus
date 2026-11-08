/** A linear axis rounded out to clean ticks: 0 / 5 / 10, never 0 / 3.7 / 7.4. */
export type Axis = { lo: number; hi: number; ticks: number[]; step: number };

export function niceAxis(min: number, max: number, count = 4, zero = false): Axis {
  let lo = zero ? Math.min(0, min) : min;
  let hi = zero ? Math.max(0, max) : max;
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return { lo: 0, hi: 1, ticks: [0, 1], step: 1 };
  if (lo === hi) {
    const pad = Math.abs(lo) / 2 || 1;
    lo -= zero && lo >= 0 ? 0 : pad;
    hi += pad;
  }
  const raw = (hi - lo) / count;
  const mag = 10 ** Math.floor(Math.log10(raw));
  const norm = raw / mag;
  const step = (norm <= 1 ? 1 : norm <= 2 ? 2 : norm <= 2.5 ? 2.5 : norm <= 5 ? 5 : 10) * mag;
  const start = Math.floor(lo / step + 1e-9) * step;
  const end = Math.ceil(hi / step - 1e-9) * step;
  const ticks: number[] = [];
  for (let t = start; t <= end + step / 2 && ticks.length < 12; t += step) ticks.push(Number(t.toPrecision(12)));
  return { lo: start, hi: end, ticks, step };
}

/** Where a value sits along an axis, 0 at its low end and 1 at its high end. */
export const fraction = (axis: Pick<Axis, "lo" | "hi">, v: number) => (axis.hi === axis.lo ? 0.5 : (v - axis.lo) / (axis.hi - axis.lo));

/** A tick in full under 10,000 and compact above (12K, 3.4M), with as many
 *  decimals as the step needs, so one axis keeps one style. */
export function tickLabel(v: number, step: number): string {
  const abs = Math.abs(v);
  if (abs >= 1e4) {
    const [div, suffix] = abs >= 1e9 ? [1e9, "B"] : abs >= 1e6 ? [1e6, "M"] : [1e3, "K"];
    return `${Number((v / div).toFixed(step / div < 1 ? 1 : 0)).toLocaleString("en-US")}${suffix}`;
  }
  let places = 0;
  while (places < 4 && Math.abs(step * 10 ** places - Math.round(step * 10 ** places)) > 1e-8) places++;
  return v.toLocaleString("en-US", { minimumFractionDigits: places, maximumFractionDigits: places });
}

/** A value for a readout or label: up to three significant decimals. */
export function valueLabel(v: number, unit?: unknown): string {
  const abs = Math.abs(v);
  const places = abs >= 100 ? 0 : abs >= 10 ? 1 : abs >= 1 ? 2 : 3;
  const text = v.toLocaleString("en-US", { maximumFractionDigits: places });
  const u = typeof unit === "string" ? unit : "";
  if (!u) return text;
  if (/^[$€£¥₹]$/.test(u)) return v < 0 ? `-${u}${text.slice(1)}` : `${u}${text}`;
  return /^[%°]/.test(u) ? `${text}${u}` : `${text} ${u}`;
}
