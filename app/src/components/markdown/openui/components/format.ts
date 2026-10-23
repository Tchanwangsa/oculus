/** A number for a readout: grouped, at most `decimals` places, with the unit
 *  before it when it is a currency sign ("$1,200") and after otherwise
 *  ("4.5%", "12 yr"). Anything not a finite number is a dash. */
export function formatNumber(value: unknown, unit?: unknown, decimals?: unknown): string {
  const n = typeof value === "number" ? value : Number(value);
  if (value === null || value === undefined || value === "" || !Number.isFinite(n)) return "—";
  const places = typeof decimals === "number" && decimals >= 0 ? Math.min(decimals, 6) : 2;
  const text = n.toLocaleString("en-US", { maximumFractionDigits: places, minimumFractionDigits: 0 });
  const u = typeof unit === "string" ? unit : "";
  if (!u) return text;
  if (/^[$€£¥₹]$/.test(u)) return n < 0 ? `-${u}${text.slice(1)}` : `${u}${text}`;
  return /^[%°]/.test(u) ? `${text}${u}` : `${text} ${u}`;
}
