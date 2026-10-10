/** Facts formatting: null is unknown ("—"), never $0. */

/** USD per million tokens: `$0`, `$0.15`, `$3`, `$2.50`. */
export function fmtPrice(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n === 0) return "$0";
  if (Number.isInteger(n)) return `$${n}`;
  if (n >= 1) return `$${n.toFixed(2)}`;
  if (n < 0.01) return `$${Number(n.toPrecision(2))}`;
  return `$${n.toFixed(3).replace(/0$/, "")}`;
}

/** A token count: `128K`, `1M`, `1.5M`. */
export function fmtTokens(n: number | null | undefined): string {
  if (n == null) return "—";
  if (n >= 1_000_000) return `${Number((n / 1_000_000).toFixed(1))}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`;
  return String(n);
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** `YYYY-MM-DD` (or `YYYY-MM`) as `Aug 2026`. */
export function fmtRelease(date: string | null | undefined): string {
  const m = date?.match(/^(\d{4})-(\d{2})/);
  const month = m ? MONTHS[Number(m[2]) - 1] : undefined;
  return m && month ? `${month} ${m[1]}` : "—";
}
