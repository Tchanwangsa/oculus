import { useLayoutEffect, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { CaretDown, CaretRight, Folder, FolderOpen, Lightning, SlidersHorizontal, type Icon } from "@phosphor-icons/react";
import type { MentionFolder } from "@/lib/db";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { fileTitle, folderTitle } from "@/lib/files/openFile";
import { displayCode } from "@/lib/format/format";
import type { ComposerCommandId } from "@/lib/harness/composerCommands";
import { SKILL_SOURCE_LABEL } from "@/lib/harness/skills";
import { PARSE_SWEEP_NOTE } from "@/lib/pipeline/parseState";
import { useParseStore } from "@/stores/sync/parseStore";
import {
  MENU_GAP,
  MENU_MAX_H,
  type ComposerMenuProps,
  type MenuAnchor,
  type MenuItem,
  type MenuRow,
} from "@/components/harness/mentions/useComposerMenu";
import { cn } from "@/lib/utils";

const MENU_WIDTH = { mention: 300, slash: 440 } as const;
/** Floor when neither side has room: overlap the line rather than shrink to nothing. */
const MENU_MIN_H = 96;

/** Each command's glyph. */
const COMMAND_ICON: Record<ComposerCommandId, Icon> = { model: SlidersHorizontal };

/**
 * `fixed` placement from the trigger's rect, clamped into the root element's
 * box on both axes; `maxHeight` is the room on the chosen side. Plain CSS
 * pixels are safe because zoom is the webview's page zoom (see docs/ui.md#gotchas).
 */
function place(anchor: MenuAnchor, drop: "up" | "down", wide: boolean): CSSProperties {
  const { clientWidth: vw, clientHeight: vh } = document.documentElement;
  const width = Math.min(wide ? MENU_WIDTH.slash : MENU_WIDTH.mention, vw - MENU_GAP * 2);
  const left = Math.max(MENU_GAP, Math.min(anchor.left, vw - width - MENU_GAP));
  const room = drop === "down" ? vh - anchor.bottom - MENU_GAP * 2 : anchor.top - MENU_GAP * 2;
  // Either side, keeps at least `MENU_MIN_H` of the popup in the window.
  const inset = Math.min(drop === "down" ? anchor.bottom + MENU_GAP : vh - anchor.top + MENU_GAP, vh - MENU_GAP - MENU_MIN_H);
  return {
    left,
    width,
    maxHeight: Math.max(MENU_MIN_H, Math.min(MENU_MAX_H, room)),
    ...(drop === "down" ? { top: inset } : { bottom: inset }),
  };
}

// `fixed` already makes the popup its rows' offset parent. A `relative` beside
// it wins in `cn` and drops the popup into the page's flow, under the window.
const POPUP = "fixed z-50 overflow-x-hidden overflow-y-auto rounded-xl border border-border bg-popover py-1 shadow-md";
const LINE = "px-3 py-2 text-[11px] leading-snug text-muted-foreground";
/** A row's left edge, and how far each opened folder above it moves it in. */
const ROW_INSET = 12;
const DEPTH_STEP = 14;
const inset = (depth: number) => ROW_INSET + depth * DEPTH_STEP;
/** The folder arrow's column; a file row leaves it blank so glyphs line up. */
const ARROW = "flex size-3.5 shrink-0 items-center justify-center";

/** A row's glyph, its name, then what trails it. */
function Row({
  item,
  subjectId,
  onToggle,
}: {
  item: MenuItem;
  subjectId: number | null;
  onToggle: (folder: MentionFolder) => void;
}): ReactNode {
  if (item.kind === "folder") {
    const { folder, open } = item;
    const Arrow = open ? CaretDown : CaretRight;
    const Glyph = open ? FolderOpen : Folder;
    return (
      <>
        {/* Not a button: the row is one, and WebKit drops a nested button's
            clicks. Prevented mousedown keeps the editor's focus, as the row's does. */}
        <span
          role="button"
          aria-label={open ? "Close folder" : "Open folder"}
          aria-expanded={open}
          title={open ? "Close folder (←)" : "Open folder (→)"}
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onToggle(folder);
          }}
          className={cn(ARROW, "rounded text-muted-foreground hover:bg-border hover:text-foreground")}
        >
          <Arrow size={9} weight="bold" />
        </span>
        <Glyph size={12} weight="fill" className="shrink-0 text-muted-foreground" />
        <span className="truncate">{folderTitle(folder.path)}</span>
      </>
    );
  }
  if (item.kind === "file") {
    const { file } = item;
    // The chip's own glyph and colour, so the row previews what a pick draws.
    const { Icon, tone } = fileReferenceAppearance(file, file.relative_path);
    return (
      <>
        <span className={ARROW} />
        <Icon size={12} className={cn("shrink-0", tone)} />
        <span className="truncate">{fileTitle(file)}</span>
        {/* Under an opened folder the subject is already above it. */}
        {subjectId == null && item.depth === 0 && (
          <span className="ml-auto shrink-0 text-[10px] text-muted-foreground/70">{displayCode(file.subject_code)}</span>
        )}
      </>
    );
  }
  if (item.kind === "command") {
    const Glyph = COMMAND_ICON[item.command.id];
    return (
      <>
        <Glyph size={12} className="shrink-0 text-muted-foreground" />
        <span className="shrink-0 text-foreground">/{item.command.name}</span>
        <span className="min-w-0 truncate text-muted-foreground">{item.command.description}</span>
      </>
    );
  }
  const { skill } = item;
  return (
    <>
      <Lightning size={12} weight="fill" className="shrink-0 text-brand" />
      <span className="shrink-0 text-foreground">/{skill.name}</span>
      <span className="min-w-0 flex-1 truncate text-muted-foreground">{skill.description}</span>
      <span className="ml-auto shrink-0 rounded-full border border-border px-1.5 text-[10px] leading-4 text-muted-foreground">
        {SKILL_SOURCE_LABEL[skill.source]}
      </span>
    </>
  );
}

/** A file can be listed twice: among the recent ones and under its folder. */
const rowKey = (row: MenuRow): string =>
  row.kind === "line"
    ? `l:${row.key}`
    : row.kind === "folder"
      ? `d:${row.folder.path}`
      : row.kind === "file"
        ? `f:${row.depth > 0 ? "in:" : ""}${row.file.id}`
        : row.kind === "command"
          ? `c:${row.command.id}`
          : `s:${row.skill.path}`;

/**
 * The composer's menu, portalled (a `fixed` child of a transformed or clipped
 * box would inherit both) and hung off the trigger character's rect: `@`'s
 * folders then files, or `/`'s "Commands" then "Skills". An `@` folder's arrow
 * opens it in place, its subfolders and files indented under it. A line says when it
 * is searching or nothing matched, and why matching files without markdown
 * cannot be picked.
 */
export function ComposerMenu({
  ref,
  groups,
  note,
  unparsed,
  index,
  onIndex,
  onPick,
  onToggle,
  anchor,
  drop,
  subjectId,
  kind,
}: ComposerMenuProps) {
  /** An app-wide parse failure, named as the real cause. */
  const latch = useParseStore((s) => s.latch);

  // Keep the highlighted row in view inside the list, without moving anything
  // around it (`scrollIntoView` would scroll the page behind the popup too).
  // A folder just opened brings the row under it into view as well, so
  // opening one at the list's foot shows that something appeared.
  const drawn = groups.reduce((n, g) => n + g.rows.length, 0);
  useLayoutEffect(() => {
    const menu = ref.current;
    const row = menu?.querySelector<HTMLElement>(`[data-menu-index="${index}"]`);
    if (!menu || !row) return;
    const under = row.getAttribute("aria-expanded") === "true" ? row.nextElementSibling : null;
    const last = under instanceof HTMLElement ? under : row;
    if (last.offsetTop + last.offsetHeight > menu.scrollTop + menu.clientHeight) {
      menu.scrollTop = last.offsetTop + last.offsetHeight - menu.clientHeight;
    }
    if (row.offsetTop < menu.scrollTop) menu.scrollTop = row.offsetTop;
  }, [index, drawn, ref]);

  if (!anchor) return null;
  let at = -1;

  return createPortal(
    <div
      ref={ref}
      style={place(anchor, drop, kind === "slash")}
      className={POPUP}
    >
      {groups.map((group, g) => (
        <div key={group.label ?? g} role="group" aria-label={group.label ?? undefined}>
          {group.label && <div className="px-3 pt-1.5 pb-1 text-[11px] text-muted-foreground">{group.label}</div>}
          {group.rows.map((item) => {
            if (item.kind === "line") {
              return (
                <div
                  key={rowKey(item)}
                  style={{ paddingLeft: inset(item.depth) }}
                  className="py-1.5 pr-3 text-[11px] text-muted-foreground"
                >
                  {item.text}
                </div>
              );
            }
            at += 1;
            const i = at;
            const tree = item.kind === "folder" || item.kind === "file";
            return (
              <button
                key={rowKey(item)}
                type="button"
                data-menu-index={i}
                aria-expanded={item.kind === "folder" ? item.open : undefined}
                // Prevented mousedown, so the editor keeps focus for the splice.
                onMouseDown={(e) => {
                  e.preventDefault();
                  onPick(item);
                }}
                onMouseEnter={() => onIndex(i)}
                style={tree ? { paddingLeft: inset(item.depth) } : undefined}
                className={cn(
                  "flex w-full items-center py-1.5 pr-3 text-left text-xs",
                  tree ? "gap-1.5" : "gap-2 pl-3",
                  i === index ? "bg-accent text-foreground" : "text-muted-foreground",
                )}
              >
                <Row item={item} subjectId={subjectId} onToggle={onToggle} />
              </button>
            );
          })}
        </div>
      ))}
      {unparsed > 0 && (
        <div className={LINE}>
          {unparsed === 1
            ? "1 matching file has no markdown, so it cannot be mentioned."
            : `${unparsed} matching files have no markdown, so they cannot be mentioned.`}
          {` ${latch ? latch.message : PARSE_SWEEP_NOTE}`}
        </div>
      )}
      {note && <div className={LINE}>{note}</div>}
    </div>,
    document.body,
  );
}
