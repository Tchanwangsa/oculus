import {
  Books,
  Globe,
  GraduationCap,
  HardDrives,
  PaintBrush,
  Sparkle,
  type Icon,
} from "@phosphor-icons/react";

import { JOBS } from "@/lib/db";

/**
 * The Settings pages and what its search finds on them. The nav
 * (`app/src/components/settings/SettingsNav.tsx`) draws the pages; a result
 * jumps to its `Section` (`app/src/pages/settings/section.tsx`). Titles are
 * the on-screen text, so a search lands where it reads.
 */

export type SettingsPageId = "canvas" | "ai" | "library" | "appearance" | "browser" | "storage";

export interface SettingsPage {
  /** Route segment under `/settings`. */
  id: SettingsPageId;
  label: string;
  icon: Icon;
  group: (typeof SETTINGS_GROUPS)[number];
}

export const SETTINGS_GROUPS = ["Integrations", "App"] as const;

/** In nav order within each group. */
export const SETTINGS_PAGES: readonly SettingsPage[] = [
  { id: "canvas", label: "Canvas", icon: GraduationCap, group: "Integrations" },
  { id: "ai", label: "AI", icon: Sparkle, group: "Integrations" },
  { id: "library", label: "Library", icon: Books, group: "Integrations" },
  { id: "appearance", label: "Appearance", icon: PaintBrush, group: "App" },
  { id: "browser", label: "Browser", icon: Globe, group: "App" },
  { id: "storage", label: "Storage", icon: HardDrives, group: "App" },
];

const PAGE_BY_ID = new Map(SETTINGS_PAGES.map((p) => [p.id, p]));

export function settingsPage(id: SettingsPageId): SettingsPage {
  return PAGE_BY_ID.get(id)!;
}

export interface SettingsEntry {
  page: SettingsPageId;
  /** A page label, a section title or a row label, as shown. */
  title: string;
  /** The `Section` title to scroll to; none lands at the page's top. */
  section?: string;
  keywords?: readonly string[];
}

/** Router `state` for a jump; the layout scrolls to `section` once it renders. */
export interface SettingsJump {
  section: string | null;
}

type Row = readonly [title: string, keywords?: readonly string[]];

function page(id: SettingsPageId, keywords?: readonly string[]): SettingsEntry {
  return { page: id, title: settingsPage(id).label, keywords };
}

/** Rows inside `section`, without an entry for the section itself. */
function rows(id: SettingsPageId, section: string, list: readonly Row[]): SettingsEntry[] {
  return list.map(([title, keywords]) => ({ page: id, title, section, keywords }));
}

/** A section's own entry, then its rows. */
function section(
  id: SettingsPageId,
  title: string,
  keywords: readonly string[],
  list: readonly Row[] = [],
): SettingsEntry[] {
  return [{ page: id, title, section: title, keywords }, ...rows(id, title, list)];
}

// A section titled like its page is covered by the page's entry.
export const SETTINGS_ENTRIES: readonly SettingsEntry[] = [
  page("canvas", ["sign in", "login", "session", "authenticate", "connect", "disconnect"]),
  ...rows("canvas", "Canvas", [
    ["Sign in without the browser", ["okta", "password", "keychain", "authenticator"]],
    ["Auto-refresh session", ["keep alive", "keepalive", "expired"]],
  ]),

  page("ai", ["agents", "models"]),
  ...section("ai", "CLI agents", ["claude code", "codex", "opencode", "antigravity", "install", "sign in"]),
  ...section("ai", "opencode providers", ["manage providers", "models", "hidden"]),
  ...section("ai", "Jobs", ["model", "reasoning"], JOBS.map((job) => [job.label] as const)),
  ...section("ai", "Antigravity approvals", ["permissions", "revoke"]),

  page("library", ["pdf"]),
  ...rows("library", "Library", [["PDFs tracked"], ["Parsed"]]),
  ...section("library", "PDF processing", ["mineru", "parse", "pdf"], [
    ["Parser", ["engine"]],
    ["MinerU API token"],
    ["Server address", ["url", "local"]],
    ["Server status"],
  ]),
  ...section("library", "Search index", ["embedding", "vectors", "reindex", "voyage"], [
    ["Embedding model"],
    ["Voyage API key"],
    ["Pages indexed"],
    ["Files indexed"],
    ["Search space"],
    ["Not indexed"],
    ["Voyage plan"],
    ["Free allowance", ["stop indexing", "limit", "budget"]],
    ["Build the index"],
  ]),
  ...section("library", "Transcription", ["whisper", "captions", "groq", "video"], [
    ["Groq API key"],
  ]),

  page("appearance"),
  ...rows("appearance", "Appearance", [["Theme", ["dark mode", "light mode", "system"]]]),

  page("browser"),
  ...rows("browser", "Browser", [
    ["Search engine", ["web search"]],
    ["Open links in", ["default browser"]],
  ]),
  ...section("browser", "History", ["clear history", "visited"]),

  page("storage", ["disk", "space", "videos"]),
  ...rows("storage", "Storage", [["Largest files"]]),
];

/**
 * Entries matching `query` (case-insensitive substring of title, keywords or
 * page label), best first: title prefix, then title, keyword, page label.
 */
export function searchSettings(query: string): SettingsEntry[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const rank = (entry: SettingsEntry): number => {
    const title = entry.title.toLowerCase();
    if (title.startsWith(q)) return 0;
    if (title.includes(q)) return 1;
    if (entry.keywords?.some((k) => k.includes(q))) return 2;
    if (settingsPage(entry.page).label.toLowerCase().includes(q)) return 3;
    return -1;
  };
  return SETTINGS_ENTRIES.map((entry) => ({ entry, score: rank(entry) }))
    .filter((r) => r.score >= 0)
    .sort((a, b) => a.score - b.score)
    .map((r) => r.entry);
}
