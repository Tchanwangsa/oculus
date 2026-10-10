import { renderToString } from "@/lib/maths";
import { frameOrigin } from "@/lib/maths/geometry";
import { COMMON_COMMANDS, MATH_COMMANDS, MATH_TABS, previewOf } from "../../tools/mathPalette";
import { commandOf } from "../../tools/mathUsage";
import type { MathView } from "./index";

/** How many commands the popover lists. */
const SHOWN = 8;
/** Px between the caret's foot and the popover. */
const GAP = 4;

interface Command {
  name: string;
  /** Rendered preview HTML, or null when it does not render. */
  preview: () => string | null;
}

let commands: Command[] | null = null;

/** Every palette entry led by a `\command`, once per name, common first. */
function allCommands(): Command[] {
  if (commands) return commands;
  const seen = new Set<string>();
  commands = [];
  for (const entry of [...MATH_TABS.flatMap((t) => t.entries), ...MATH_COMMANDS]) {
    const name = commandOf(entry.template);
    if (!name || seen.has(name)) continue;
    seen.add(name);
    let html: string | null | undefined;
    commands.push({
      name,
      preview: () => {
        if (html !== undefined) return html;
        try {
          html = renderToString(previewOf(entry), { throwOnError: true });
        } catch {
          html = null;
        }
        return html;
      },
    });
  }
  const rank = (c: Command) => (COMMON_COMMANDS.has(c.name) ? 0 : 1);
  commands.sort((a, b) => rank(a) - rank(b) || a.name.length - b.name.length || a.name.localeCompare(b.name));
  return commands;
}

/** The pending command's popover, and what it lists. */
function fill(popover: HTMLElement, pending: string) {
  const typed = `\\${pending}`;
  popover.replaceChildren();
  const head = document.createElement("span");
  head.className = "cm-math-view-popover-typed";
  head.textContent = typed;
  popover.append(head);
  const matches = allCommands().filter((c) => c.name.startsWith(typed) && c.name !== typed);
  const exact = allCommands().find((c) => c.name === typed);
  for (const c of [...(exact ? [exact] : []), ...matches].slice(0, SHOWN)) {
    const row = document.createElement("span");
    row.className = "cm-math-view-popover-row";
    const preview = document.createElement("span");
    preview.className = "cm-math-view-popover-preview";
    const html = c.preview();
    if (html) preview.innerHTML = html;
    const name = document.createElement("span");
    name.className = "cm-math-view-popover-name";
    name.textContent = c.name;
    row.append(preview, name);
    popover.append(row);
  }
}

/**
 * In command mode, the `\name` being typed in a popover under the caret,
 * with the palette's commands that start with it, read-only: the model
 * commits what was typed.
 */
export function drawPopover(view: MathView) {
  const { field, popover, measured, frame, dom } = view;
  const id = field.head;
  const x = measured.x[id];
  if (field.mode !== "command" || field.pending == null || !Number.isFinite(x)) {
    popover.hidden = true;
    delete popover.dataset.pending;
    return;
  }
  if (popover.dataset.pending !== field.pending) {
    fill(popover, field.pending);
    popover.dataset.pending = field.pending;
  }
  popover.hidden = false;
  const o = frameOrigin(frame);
  const box = dom.getBoundingClientRect();
  popover.style.left = `${o.x + x - box.left - dom.clientLeft}px`;
  popover.style.top = `${o.y + measured.bottom[id] - box.top - dom.clientTop + GAP}px`;
}
