/**
 * The start of a text file as a Drive card draws it: a few dozen lines, with
 * markdown lightly rendered (frontmatter dropped, headings and list items
 * marked, inline markup removed) so the card reads like the page, without
 * the note renderer or maths per card.
 */

import type { DriveKind } from "./filters";

export type PreviewFormat = "markdown" | "plain" | "code";

export type PreviewStyle = "h1" | "h2" | "h3" | "body" | "bullet" | "quote" | "code" | "gap";

export interface PreviewLine {
  style: PreviewStyle;
  text: string;
  /** A list item's marker ("•", "1.", "☐"); its nesting depth is `depth`. */
  marker?: string;
  depth?: number;
}

/** More than a card can show at any width. */
const MAX_LINES = 40;
const MAX_CHARS = 240;
/** Only the head of the file is looked at. */
const PREVIEW_CHARS = 8 * 1024;

const ext = (name: string) => (name.includes(".") ? name.slice(name.lastIndexOf(".") + 1).toLowerCase() : "");

/** How a file previews, or null when it is not text a card should show. */
export function previewFormat(name: string, kind: DriveKind): PreviewFormat | null {
  const e = ext(name);
  if (e === "md" || e === "markdown") return "markdown";
  if (e === "txt") return "plain";
  if (kind === "code") return "code";
  return null;
}

export function textPreview(text: string, format: PreviewFormat): PreviewLine[] {
  const lines = text.slice(0, PREVIEW_CHARS).replace(/\r\n?/g, "\n").split("\n");
  if (format === "markdown") return markdownLines(lines);
  const style: PreviewStyle = format === "code" ? "code" : "body";
  return trimGaps(lines.slice(0, MAX_LINES).map((line) => {
    const clipped = line.replace(/\t/g, "  ").slice(0, MAX_CHARS);
    return clipped.trim() ? { style, text: clipped } : { style: "gap", text: "" };
  }));
}

function markdownLines(all: string[]): PreviewLine[] {
  const out: PreviewLine[] = [];
  let lines = all;
  if (lines[0]?.trim() === "---") {
    const end = lines.findIndex((line, i) => i > 0 && /^(---|\.\.\.)\s*$/.test(line));
    if (end > 0) lines = lines.slice(end + 1);
  }
  let fence: string | null = null;
  let comment = false;
  for (const raw of lines) {
    if (out.length >= MAX_LINES) break;
    if (fence) {
      if (raw.trim().startsWith(fence)) fence = null;
      else out.push({ style: "code", text: raw.replace(/\t/g, "  ").slice(0, MAX_CHARS) });
      continue;
    }
    if (comment) {
      if (raw.includes("-->")) comment = false;
      continue;
    }
    const line = raw.replace(/<!--.*?-->/g, "");
    if (line.includes("<!--")) { comment = true; continue; }
    const opener = /^\s*(`{3,}|~{3,})/.exec(line);
    if (opener) { fence = opener[1]; continue; }
    const trimmed = line.trim();
    if (!trimmed || trimmed === "$$") { out.push({ style: "gap", text: "" }); continue; }
    if (/^([-*_])(\s*\1){2,}$/.test(trimmed)) continue;
    if (trimmed.includes("|") && /^\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?$/.test(trimmed)) continue;
    const heading = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(trimmed);
    if (heading) {
      const level = heading[1].length;
      out.push({ style: level === 1 ? "h1" : level === 2 ? "h2" : "h3", text: inline(heading[2]) });
      continue;
    }
    const item = /^(\s*)([-*+]|\d+[.)])\s+(?:\[([ xX])\]\s+)?(.*)$/.exec(line);
    if (item) {
      const marker = item[3] !== undefined ? (item[3] === " " ? "☐" : "☑") : /\d/.test(item[2]) ? item[2] : "•";
      const depth = Math.min(3, Math.floor(item[1].replace(/\t/g, "    ").length / 2));
      out.push({ style: "bullet", marker, depth, text: inline(item[4]) });
      continue;
    }
    if (trimmed.startsWith(">")) {
      out.push({ style: "quote", text: inline(trimmed.replace(/^(>\s?)+/, "")) });
      continue;
    }
    const text = inline(trimmed.startsWith("|") ? trimmed.replace(/^\||\|$/g, "").split("|").map((cell) => cell.trim()).join("   ") : trimmed);
    if (text) out.push({ style: "body", text });
  }
  return trimGaps(out);
}

/** Markdown's inline syntax removed, leaving the words it marks. */
export function inline(text: string): string {
  return text
    // Escaped characters hide in the private-use area until the markup is gone.
    .replace(/\\([\\`*_{}[\]()#+\-.!|~>])/g, (_, c: string) => String.fromCharCode(0xe000 + c.charCodeAt(0)))
    .replace(/<\/?[a-zA-Z][^>]*>/g, "")
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, target: string, alias?: string) => alias ?? target)
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\[[^\]]*\]/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/(\*\*|__)(?=\S)(.+?)(?<=\S)\1/g, "$2")
    .replace(/~~(?=\S)(.+?)(?<=\S)~~/g, "$1")
    .replace(/(^|[^\w*])\*(?=\S)(.+?)(?<=\S)\*(?!\*)/g, "$1$2")
    .replace(/(^|[^\w])_(?=\S)(.+?)(?<=\S)_(?!\w)/g, "$1$2")
    .replace(/[\ue000-\ue07f]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xe000))
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .trim()
    .slice(0, MAX_CHARS);
}

/** No gap at either end, and never two in a row. */
function trimGaps(lines: PreviewLine[]): PreviewLine[] {
  const out: PreviewLine[] = [];
  for (const line of lines) {
    if (line.style === "gap" && (!out.length || out[out.length - 1].style === "gap")) continue;
    out.push(line);
  }
  while (out.length && out[out.length - 1].style === "gap") out.pop();
  return out;
}
