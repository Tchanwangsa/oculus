import { parseItemMeta } from "./meta";
import type { HarnessItem } from "./types";

/** How long a reasoning row streamed, in ms (`ThoughtClock` in Rust, or
 *  opencode's part clock); null on rows written before it was kept. */
export function thoughtMs(item: HarnessItem): number | null {
  const ms = parseItemMeta(item).ms;
  return typeof ms === "number" && Number.isFinite(ms) && ms >= 0 ? ms : null;
}

/** "Thought for 12s", "Thought for 1m 5s"; "Thought" when untimed. */
export function thoughtLabel(ms: number | null): string {
  if (ms == null) return "Thought";
  const s = Math.max(1, Math.round(ms / 1000));
  if (s < 60) return `Thought for ${s}s`;
  const m = Math.floor(s / 60);
  return `Thought for ${m}m${s % 60 ? ` ${s % 60}s` : ""}`;
}

/** Markdown and maths delimiters off, so a line reads as plain text. */
function plain(s: string): string {
  return s
    .replace(/\$\$?([^$]*)\$\$?/g, "$1")
    .replace(/[*_`]+/g, "")
    .replace(/^\s*(?:#+|[-+>]|\d+[.)])\s+/, "")
    .trim();
}

/** Short enough that a sentence still being written would flicker word by word. */
const SETTLE = 40;

/** The latest sentence of a thought still streaming, for its one-line row:
 *  the one being written once it is long enough to read, else the last
 *  finished one. */
export function latestThought(text: string): string {
  const lines = text.split("\n").map(plain).filter(Boolean);
  const sentences = (line: string | undefined) => (line ? line.split(/(?<=[.!?:])\s+/).filter(Boolean) : []);
  const last = sentences(lines[lines.length - 1]);
  const current = last[last.length - 1] ?? "";
  if (/[.!?:]$/.test(current) || current.length >= SETTLE) return current;
  const before = last.length > 1 ? last : sentences(lines[lines.length - 2]);
  return (last.length > 1 ? before[before.length - 2] : before[before.length - 1]) ?? current;
}
