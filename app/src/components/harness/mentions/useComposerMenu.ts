import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import type { MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import {
  countUnparsedMentionMatches,
  listMentionFolder,
  searchMentionFiles,
  searchMentionFolders,
  type MentionFile,
  type MentionFolder,
  type MentionFolderContents,
} from "@/lib/db";
import { searchSlashItems, type ComposerCommand, type ComposerCommandId } from "@/lib/harness/composerCommands";
import type { Skill } from "@/lib/harness/skills";
import { detectTrigger, type ComposerTrigger } from "./trigger";

/** Pixel gap from the anchor line and viewport edge; `./ComposerMenu.tsx` uses it too. */
export const MENU_GAP = 8;
export const MENU_MAX_H = 288;

/** How long a library lookup may take before the menu says it is searching,
 *  so a fast one never flashes the line. */
const SLOW_LOOKUP_MS = 150;

/** The trigger character's left edge and its line's top/bottom, in viewport pixels. */
export interface MenuAnchor {
  left: number;
  top: number;
  bottom: number;
}

/** Reserve room for the full list so filtering to one line cannot flip it. */
export function menuDirection(anchor: MenuAnchor, height: number): "up" | "down" {
  const below = height - anchor.bottom - MENU_GAP * 2;
  const above = anchor.top - MENU_GAP * 2;
  return below >= MENU_MAX_H || below >= above ? "down" : "up";
}

/** One pickable row. `depth` is how many opened folders an `@` row sits
 *  under; `open` is a folder showing what it holds. */
export type MenuItem =
  | { kind: "folder"; folder: MentionFolder; depth: number; open: boolean }
  | { kind: "file"; file: MentionFile; depth: number }
  | { kind: "command"; command: ComposerCommand }
  | { kind: "skill"; skill: Skill };

/** A row as drawn: a pickable item, or a line under an opened folder that
 *  says it is empty or how many files it left out. */
export type MenuRow = MenuItem | { kind: "line"; key: string; text: string; depth: number };

/** Rows under an optional label; the `/` menu has "Commands" then "Skills". */
export interface MenuGroup {
  label: string | null;
  rows: MenuRow[];
}

/** The folders opened in the `@` menu, by path, each with what it holds. */
export type OpenedFolders = ReadonlyMap<string, MentionFolderContents>;

const NONE_OPENED: OpenedFolders = new Map();

/** The rows a group's highlight and Enter move over. */
export const pickable = (rows: readonly MenuRow[]): MenuItem[] =>
  rows.filter((row): row is MenuItem => row.kind !== "line");

/** A folder's row and, when it is opened, what it holds one level in:
 *  subfolders (each opening the same way), then files, then a line if there
 *  is nothing or more than was listed. */
function folderRows(folder: MentionFolder, depth: number, opened: OpenedFolders): MenuRow[] {
  const inside = opened.get(folder.path);
  const rows: MenuRow[] = [{ kind: "folder", folder, depth, open: inside != null }];
  if (!inside) return rows;
  for (const sub of inside.folders) rows.push(...folderRows(sub, depth + 1, opened));
  for (const file of inside.files) rows.push({ kind: "file", file, depth: depth + 1 });
  const line = (text: string): MenuRow => ({ kind: "line", key: `${folder.path}#line`, text, depth: depth + 1 });
  if (inside.folders.length + inside.files.length === 0) rows.push(line("Nothing here to mention."));
  else if (inside.more > 0) rows.push(line(`${inside.more} more. Type a name to find one.`));
  return rows;
}

/**
 * The trigger's rect off the live selection; `back` is query length + 1. Anchored
 * to the trigger character, not the caret, so the menu doesn't slide as the
 * query grows and a re-measure is idempotent. Measured over the range
 * trigger→caret because WebKit can return an all-zero rect for a collapsed
 * Range; the character is checked, not trusted (the DOM holds ZWSP guards).
 * Fallbacks never yield 0,0.
 */
function measureAnchor(back: number, char: string): MenuAnchor | null {
  const sel = window.getSelection();
  const node = sel?.focusNode;
  if (!sel || !node) return null;

  const range = document.createRange();
  const at = sel.focusOffset - back;
  if (node.nodeType === Node.TEXT_NODE && at >= 0 && (node as Text).data[at] === char) {
    range.setStart(node, at);
    range.setEnd(node, sel.focusOffset);
  } else {
    range.setStart(node, sel.focusOffset);
    range.collapse(true);
  }

  const rects = range.getClientRects();
  let rect: DOMRect | null = rects.length > 0 ? rects[0] : null;
  if (!rect) {
    const bounds = range.getBoundingClientRect();
    if (bounds.height > 0) rect = bounds;
  }
  if (!rect) {
    const el = node.nodeType === Node.TEXT_NODE ? node.parentElement : (node as Element);
    const bounds = el?.getBoundingClientRect();
    if (bounds && (bounds.height > 0 || bounds.width > 0)) rect = bounds;
  }
  return rect ? { left: rect.left, top: rect.top, bottom: rect.bottom } : null;
}

const triggerChar = (t: ComposerTrigger) => (t.kind === "mention" ? "@" : "/");

/** The groups and the line under them for a trigger: what `ComposerMenu`
 *  draws. Pure, so the copy and the order are tested without a DOM. */
export function menuContent(input: {
  trigger: ComposerTrigger | null;
  /** The library lookup for the current `@` token; null until it answers. */
  library: { folders: MentionFolder[]; files: MentionFile[]; unparsed: number } | null;
  /** The lookup has run past `SLOW_LOOKUP_MS`. */
  slow: boolean;
  commands: readonly ComposerCommand[];
  /** Null while the catalogue loads. */
  skills: readonly Skill[] | null;
  /** The `@` folders opened so far; none when left out. */
  opened?: OpenedFolders;
}): { groups: MenuGroup[]; note: string | null; unparsed: number } {
  const { trigger, library, slow, commands, skills, opened = NONE_OPENED } = input;
  if (!trigger) return { groups: [], note: null, unparsed: 0 };

  if (trigger.kind === "slash") {
    const found = searchSlashItems(commands, skills ?? [], trigger.query);
    const groups: MenuGroup[] = [];
    if (found.commands.length) {
      groups.push({ label: "Commands", rows: found.commands.map((command) => ({ kind: "command", command })) });
    }
    if (found.skills.length) {
      groups.push({ label: "Skills", rows: found.skills.map((skill) => ({ kind: "skill", skill })) });
    }
    const note = skills == null ? "Searching skills…" : groups.length ? null : "No matching command.";
    return { groups, note, unparsed: 0 };
  }

  if (!library) return { groups: [], note: slow ? "Searching the library…" : null, unparsed: 0 };
  const rows: MenuRow[] = [
    ...library.folders.flatMap((folder) => folderRows(folder, 0, opened)),
    ...library.files.map((file): MenuRow => ({ kind: "file", file, depth: 0 })),
  ];
  if (rows.length) return { groups: [{ label: null, rows }], note: null, unparsed: 0 };
  if (library.unparsed > 0) return { groups: [], note: null, unparsed: library.unparsed };
  // A query that has run on past one word is more likely prose than a filename.
  const words = trigger.query.trim().split(/\s+/).filter(Boolean).length;
  return { groups: [], note: words <= 1 ? "No matching files or folders." : null, unparsed: 0 };
}

export interface ComposerMenuProps {
  ref: RefObject<HTMLDivElement | null>;
  groups: MenuGroup[];
  /** A line under the rows, or in their place: searching, or nothing matched. */
  note: string | null;
  /** Matching files with no markdown yet; the menu explains them instead. */
  unparsed: number;
  /** Index into the groups' rows, read in order. */
  index: number;
  onIndex: (i: number) => void;
  onPick: (item: MenuItem) => void;
  /** Opens or closes an `@` folder under its row. */
  onToggle: (folder: MentionFolder) => void;
  /** Null when nothing could be measured or nothing is open; the menu then draws nothing. */
  anchor: MenuAnchor | null;
  drop: "up" | "down";
  /** Null is the whole library — the only case a file row names its subject. */
  subjectId: number | null;
  /** `slash` menus are wider: their rows carry a description. */
  kind: ComposerTrigger["kind"] | null;
}

/**
 * The composer's one menu: `@` for library files and folders, `/` at the
 * start of a word for commands and skills. An `@` folder opens in place
 * (ArrowRight, or its arrow) to show what it holds, and ArrowLeft closes it
 * or steps out to it. The list itself is
 * `./ComposerMenu.tsx`; what the box does with its text is the caller's.
 * `keyDown` runs first in the box's key handler and `preventDefault`s what it
 * claims, so the caller's Enter checks `defaultPrevented`.
 */
export function useComposerMenu({
  subjectId,
  input,
  skills,
  commands,
  onCommand,
}: {
  /** A subject, or null for the whole library. */
  subjectId: number | null;
  input: RefObject<MentionInputHandle | null>;
  /** The installed skills; null while the catalogue loads. */
  skills: readonly Skill[] | null;
  commands: readonly ComposerCommand[];
  /** A picked command; its `/word` is already gone from the box. */
  onCommand: (id: ComposerCommandId) => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [trigger, setTrigger] = useState<ComposerTrigger | null>(null);
  /** Beside `trigger`, not in it, so a re-measure doesn't re-run the lookup. */
  const [anchor, setAnchor] = useState<MenuAnchor | null>(null);
  const [library, setLibrary] = useState<{ folders: MentionFolder[]; files: MentionFile[]; unparsed: number } | null>(null);
  const [opened, setOpened] = useState<OpenedFolders>(NONE_OPENED);
  /** Bumped when the `@` menu shuts, so a listing still on its way is dropped. */
  const openedFor = useRef(0);
  const [slow, setSlow] = useState(false);
  const [index, setIndex] = useState(0);
  const [drop, setDrop] = useState<"up" | "down">("down");
  // Guards against a slow lookup overwriting a newer list.
  const latest = useRef("");
  /** The token Escape closed. The Escape keyup reports the same text, which
   *  would reopen it; it stays shut until the text or caret leaves it. */
  const dismissed = useRef<ComposerTrigger | null>(null);

  const mention = trigger?.kind === "mention" ? trigger.query : null;
  useEffect(() => {
    if (mention == null) {
      latest.current = "";
      setLibrary(null);
      setSlow(false);
      openedFor.current += 1;
      setOpened(NONE_OPENED);
      return;
    }
    const token = `${subjectId ?? ""} ${mention}`;
    latest.current = token;
    const timer = setTimeout(() => {
      if (latest.current === token) setSlow(true);
    }, SLOW_LOOKUP_MS);
    Promise.all([searchMentionFolders(subjectId, mention), searchMentionFiles(subjectId, mention)])
      .then(async ([folders, files]) => {
        if (latest.current !== token) return;
        // Counted only when empty, to explain it — not a second query per key.
        const unparsed =
          folders.length + files.length === 0
            ? await countUnparsedMentionMatches(subjectId, mention).catch(() => 0)
            : 0;
        if (latest.current !== token) return;
        clearTimeout(timer);
        setSlow(false);
        setLibrary({ folders, files, unparsed });
        setIndex(0);
      })
      .catch(() => {});
    return () => clearTimeout(timer);
  }, [mention, subjectId]);

  // A subject change re-scopes what `@` may reach, so the open list is stale.
  useEffect(() => {
    setTrigger(null);
    setAnchor(null);
  }, [subjectId]);

  // Re-measure (not close) on scroll: the box itself scrolls while typing, and
  // a `fixed` popup doesn't follow. Capture phase, since scroll doesn't bubble.
  useEffect(() => {
    if (!trigger) return;
    const remeasure = () => setAnchor(measureAnchor(trigger.query.length + 1, triggerChar(trigger)));
    window.addEventListener("scroll", remeasure, true);
    return () => window.removeEventListener("scroll", remeasure, true);
  }, [trigger]);

  const content = useMemo(
    () => menuContent({ trigger, library, slow, commands, skills, opened }),
    [trigger, library, slow, commands, skills, opened],
  );
  const items = useMemo(() => content.groups.flatMap((g) => pickable(g.rows)), [content]);
  const shown = trigger !== null && (items.length > 0 || content.note !== null || content.unparsed > 0);

  // A new `/` query re-ranks the rows, so the highlight goes back to the top.
  const slash = trigger?.kind === "slash" ? trigger.query : null;
  useEffect(() => setIndex(0), [slash]);
  const at = Math.min(index, Math.max(0, items.length - 1));

  // Choose from the available room, independent of results or status lines.
  // Before paint, so even an initially empty menu opens on the correct side.
  useLayoutEffect(() => {
    if (!shown || !anchor) return;
    setDrop(menuDirection(anchor, document.documentElement.clientHeight));
  }, [shown, anchor]);

  /** The editor's `onEdit`. The anchor is measured here, inside the event, so the
   *  selection is live and the menu lands placed in the same commit. `commit`'s
   *  calls report before re-render but never leave a token open to place.
   *  `masked` hides picked skills, which read as `/name`. */
  function track(_text: string, caret: number, masked: string) {
    let next = detectTrigger(masked, caret);
    const shut = dismissed.current;
    if (shut && next && shut.kind === next.kind && shut.query === next.query && shut.start === next.start) next = null;
    else dismissed.current = null;
    // The box reports on every keyup, arrows included. Keeping the same token
    // keeps the lookup and the highlighted row from resetting.
    setTrigger((prev) =>
      prev && next && prev.kind === next.kind && prev.query === next.query && prev.start === next.start ? prev : next,
    );
    setAnchor(next ? measureAnchor(next.query.length + 1, triggerChar(next)) : null);
  }

  function close() {
    setTrigger(null);
    setAnchor(null);
  }

  /** Opens a folder under its row, reading one level; closes an open one. */
  function toggle(folder: MentionFolder) {
    if (opened.has(folder.path)) {
      setOpened((prev) => {
        const next = new Map(prev);
        next.delete(folder.path);
        return next;
      });
      return;
    }
    const session = openedFor.current;
    listMentionFolder(folder.path)
      .then((inside) => {
        if (openedFor.current === session) setOpened((prev) => new Map(prev).set(folder.path, inside));
      })
      .catch(() => {});
  }

  function pick(item: MenuItem) {
    const box = input.current;
    if (!trigger || !box) return;
    if (item.kind === "file") box.insertMention(trigger.start, item.file.relative_path);
    else if (item.kind === "folder") box.insertMention(trigger.start, item.folder.path);
    else if (item.kind === "skill") box.insertSkill(trigger.start, item.skill.name);
    else {
      box.removeToken(trigger.start);
      onCommand(item.command.id);
    }
    close();
  }

  /** Claims nothing while the menu is shut, and only Escape while it shows a
   *  line but no rows, so the caller keeps its Enter: `/word` that matches
   *  nothing is sent as typed. */
  function keyDown(e: KeyboardEvent<HTMLElement>) {
    if (!shown) return;
    if (e.key === "Escape") {
      e.preventDefault();
      dismissed.current = trigger;
      close();
      return;
    }
    if (!items.length) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex((at + 1) % items.length);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex((at - 1 + items.length) % items.length);
      return;
    }
    const row = items[at];
    const plain = !e.shiftKey && !e.metaKey && !e.altKey && !e.ctrlKey;
    // A folder tree's keys: right opens, then steps in; left closes, or steps
    // out to the folder a row is under. Elsewhere the arrows move the caret.
    if (plain && e.key === "ArrowRight" && row.kind === "folder") {
      e.preventDefault();
      if (!row.open) toggle(row.folder);
      else setIndex(Math.min(at + 1, items.length - 1));
      return;
    }
    if (plain && e.key === "ArrowLeft" && row.kind === "folder" && row.open) {
      e.preventDefault();
      toggle(row.folder);
      return;
    }
    if (plain && e.key === "ArrowLeft" && (row.kind === "folder" || row.kind === "file") && row.depth > 0) {
      e.preventDefault();
      for (let i = at - 1; i >= 0; i -= 1) {
        const above = items[i];
        if (above.kind === "folder" && above.depth < row.depth) {
          setIndex(i);
          break;
        }
      }
      return;
    }
    if (e.key === "Enter" || e.key === "Tab") {
      e.preventDefault();
      pick(items[at]);
    }
  }

  return {
    track,
    close,
    keyDown,
    /** Whether a menu is showing, so the caller's own arrow keys stand aside. */
    shown,
    /** Spread into `<ComposerMenu {...menu} />`. */
    menu: {
      ref: menuRef,
      groups: content.groups,
      note: content.note,
      unparsed: content.unparsed,
      index: at,
      onIndex: setIndex,
      onPick: pick,
      onToggle: toggle,
      anchor: shown ? anchor : null,
      drop,
      subjectId,
      kind: trigger?.kind ?? null,
    } satisfies ComposerMenuProps,
  };
}
