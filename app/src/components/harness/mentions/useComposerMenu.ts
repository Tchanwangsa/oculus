import { useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import type { MentionInputHandle } from "@/components/harness/mentions/MentionInput";
import {
  countUnparsedMentionMatches,
  searchMentionFiles,
  searchMentionFolders,
  type MentionFile,
  type MentionFolder,
} from "@/lib/db";
import { searchSlashItems, type ComposerCommand, type ComposerCommandId } from "@/lib/harness/composerCommands";
import type { Skill } from "@/lib/harness/skills";
import { detectTrigger, type ComposerTrigger } from "./trigger";

/** Pixel gap from the anchor line and viewport edge; `./ComposerMenu.tsx` uses it too. */
export const MENU_GAP = 8;

/** How long a library lookup may take before the menu says it is searching,
 *  so a fast one never flashes the line. */
const SLOW_LOOKUP_MS = 150;

/** The trigger character's left edge and its line's top/bottom, in viewport pixels. */
export interface MenuAnchor {
  left: number;
  top: number;
  bottom: number;
}

/** One pickable row. */
export type MenuItem =
  | { kind: "folder"; folder: MentionFolder }
  | { kind: "file"; file: MentionFile }
  | { kind: "command"; command: ComposerCommand }
  | { kind: "skill"; skill: Skill };

/** Rows under an optional label; the `/` menu has "Commands" then "Skills". */
export interface MenuGroup {
  label: string | null;
  items: MenuItem[];
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
}): { groups: MenuGroup[]; note: string | null; unparsed: number } {
  const { trigger, library, slow, commands, skills } = input;
  if (!trigger) return { groups: [], note: null, unparsed: 0 };

  if (trigger.kind === "slash") {
    const found = searchSlashItems(commands, skills ?? [], trigger.query);
    const groups: MenuGroup[] = [];
    if (found.commands.length) {
      groups.push({ label: "Commands", items: found.commands.map((command) => ({ kind: "command", command })) });
    }
    if (found.skills.length) {
      groups.push({ label: "Skills", items: found.skills.map((skill) => ({ kind: "skill", skill })) });
    }
    const note = skills == null ? "Searching skills…" : groups.length ? null : "No matching command.";
    return { groups, note, unparsed: 0 };
  }

  if (!library) return { groups: [], note: slow ? "Searching the library…" : null, unparsed: 0 };
  const items: MenuItem[] = [
    ...library.folders.map((folder): MenuItem => ({ kind: "folder", folder })),
    ...library.files.map((file): MenuItem => ({ kind: "file", file })),
  ];
  if (items.length) return { groups: [{ label: null, items }], note: null, unparsed: 0 };
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
 * start of a word for commands and skills. The list itself is
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
    () => menuContent({ trigger, library, slow, commands, skills }),
    [trigger, library, slow, commands, skills],
  );
  const items = useMemo(() => content.groups.flatMap((g) => g.items), [content]);
  const shown = trigger !== null && (items.length > 0 || content.note !== null || content.unparsed > 0);

  // A new `/` query re-ranks the rows, so the highlight goes back to the top.
  const slash = trigger?.kind === "slash" ? trigger.query : null;
  useEffect(() => setIndex(0), [slash]);
  const at = Math.min(index, Math.max(0, items.length - 1));

  // Open down unless the list's measured height won't fit under the trigger's
  // line (the composer sits mid-page and at the bottom). Before paint, so it
  // never flashes. The root element's box is the viewport the popup lives in.
  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!shown || !menu || !anchor) return;
    const needed = menu.offsetHeight + MENU_GAP;
    const height = document.documentElement.clientHeight;
    setDrop(height - anchor.bottom >= needed || anchor.top < needed ? "down" : "up");
  }, [shown, items.length, content.note, anchor]);

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
      anchor: shown ? anchor : null,
      drop,
      subjectId,
      kind: trigger?.kind ?? null,
    } satisfies ComposerMenuProps,
  };
}
