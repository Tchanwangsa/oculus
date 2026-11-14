import { useLayoutEffect, type CSSProperties, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Folder, Lightning, SlidersHorizontal, type Icon } from "@phosphor-icons/react";
import { fileReferenceAppearance } from "@/lib/files/referenceAppearance";
import { fileTitle } from "@/lib/files/openFile";
import { displayCode, humanizeSlug } from "@/lib/format/format";
import type { ComposerCommandId } from "@/lib/harness/composerCommands";
import { SKILL_SOURCE_LABEL } from "@/lib/harness/skills";
import { PARSE_SWEEP_NOTE } from "@/lib/pipeline/parseState";
import { useParseStore } from "@/stores/sync/parseStore";
import {
  MENU_GAP,
  type ComposerMenuProps,
  type MenuAnchor,
  type MenuItem,
} from "@/components/harness/mentions/useComposerMenu";
import { cn } from "@/lib/utils";

const MENU_WIDTH = { mention: 300, slash: 440 } as const;
const MENU_MAX_H = 288;
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

const POPUP = "fixed z-50 rounded-xl border border-border bg-popover shadow-md";
const LINE = "px-3 py-2 text-[11px] leading-snug text-muted-foreground";

/** A row's glyph, its name, then what trails it. */
function Row({
  item,
  subjectId,
}: {
  item: MenuItem;
  subjectId: number | null;
}): ReactNode {
  if (item.kind === "folder") {
    const { folder } = item;
    return (
      <>
        <Folder size={12} weight="fill" className="shrink-0 text-muted-foreground" />
        <span className="truncate">{subjectId == null ? displayCode(folder.subject_code) : humanizeSlug(folder.name)}</span>
      </>
    );
  }
  if (item.kind === "file") {
    const { file } = item;
    // The chip's own glyph and colour, so the row previews what a pick draws.
    const { Icon, tone } = fileReferenceAppearance(file, file.relative_path);
    return (
      <>
        <Icon size={12} className={cn("shrink-0", tone)} />
        <span className="truncate">{fileTitle(file)}</span>
        {subjectId == null && (
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

const itemKey = (item: MenuItem): string =>
  item.kind === "folder"
    ? `d:${item.folder.path}`
    : item.kind === "file"
      ? `f:${item.file.id}`
      : item.kind === "command"
        ? `c:${item.command.id}`
        : `s:${item.skill.path}`;

/**
 * The composer's menu, portalled (a `fixed` child of a transformed or clipped
 * box would inherit both) and hung off the trigger character's rect: `@`'s
 * folders then files, or `/`'s "Commands" then "Skills". A line says when it
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
  anchor,
  drop,
  subjectId,
  kind,
}: ComposerMenuProps) {
  /** An app-wide parse failure, named as the real cause. */
  const latch = useParseStore((s) => s.latch);

  // Keep the highlighted row in view inside the list, without moving anything
  // around it (`scrollIntoView` would scroll the page behind the popup too).
  useLayoutEffect(() => {
    const menu = ref.current;
    const row = menu?.querySelector<HTMLElement>(`[data-menu-index="${index}"]`);
    if (!menu || !row) return;
    if (row.offsetTop < menu.scrollTop) menu.scrollTop = row.offsetTop;
    else if (row.offsetTop + row.offsetHeight > menu.scrollTop + menu.clientHeight) {
      menu.scrollTop = row.offsetTop + row.offsetHeight - menu.clientHeight;
    }
  }, [index, ref]);

  if (!anchor) return null;
  let at = -1;

  return createPortal(
    <div
      ref={ref}
      style={place(anchor, drop, kind === "slash")}
      className={cn(POPUP, "relative overflow-x-hidden overflow-y-auto py-1")}
    >
      {groups.map((group, g) => (
        <div key={group.label ?? g} role="group" aria-label={group.label ?? undefined}>
          {group.label && <div className="px-3 pt-1.5 pb-1 text-[11px] text-muted-foreground">{group.label}</div>}
          {group.items.map((item) => {
            at += 1;
            const i = at;
            return (
              <button
                key={itemKey(item)}
                type="button"
                data-menu-index={i}
                // Prevented mousedown, so the editor keeps focus for the splice.
                onMouseDown={(e) => {
                  e.preventDefault();
                  onPick(item);
                }}
                onMouseEnter={() => onIndex(i)}
                className={cn(
                  "flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs",
                  i === index ? "bg-accent text-foreground" : "text-muted-foreground",
                )}
              >
                <Row item={item} subjectId={subjectId} />
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
