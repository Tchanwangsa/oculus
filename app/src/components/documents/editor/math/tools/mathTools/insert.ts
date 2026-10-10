import { snippet } from "@codemirror/autocomplete";
import type { EditorView } from "@codemirror/view";

import { noteHost } from "@/components/documents/editor/core/host";
import { activeMathField, visualToggle } from "../../field/mathField";
import { snippetTemplate, type MathEntry } from "../mathPalette";
import { recordUse } from "../mathUsage";

/** A template as the LaTeX it inserts, fields left empty. */
export const sourceOf = (template: string) => template.replace(/[#$]\{[^{}]*\}/g, "");

/** The note's subject, whose recents lead the field's picks. */
export const subjectOf = (view: EditorView) => view.state.facet(noteHost).subjectId;

/** A palette entry into the open field (slots become placeholders), else
 *  into the note as a snippet; either way it counts as used. */
export function insertEntry(view: EditorView, entry: MathEntry) {
  const field = visualToggle(view.state) === "tex" ? activeMathField(view) : null;
  if (field) field.insertTemplate(entry.template);
  else {
    const { from, to } = view.state.selection.main;
    snippet(snippetTemplate(entry.template))(view, null, from, to);
  }
  recordUse(entry, subjectOf(view));
}
