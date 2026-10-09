import { Compartment, Facet, type EditorState } from "@codemirror/state";
import type { SyntaxNode } from "@lezer/common";
import { ancestorAt } from "../syntax/syntax";

/** What the editor needs from the page around it: picture URLs, where a
 *  link goes, and the subject and note path `@` searches by (`mentions.ts`).
 *  Reconfigured through `hostCompartment` when the data dir or the note's
 *  path changes, which re-resolves every picture. */
export interface NoteHost {
  imageSrc(src: string): string;
  openLink(href: string): void;
  /** `@` searches this subject's files; null searches the whole library. */
  subjectId: number | null;
  /** Library path of the note, `courses/<code>/documents/<title>.md`, which
   *  `@` leaves out; null for an editor with no file behind it. */
  notePath: string | null;
}

export const noteHost = Facet.define<NoteHost, NoteHost>({
  combine: (values) =>
    values[0] ?? { imageSrc: () => "", openLink: () => {}, subjectId: null, notePath: null },
});

export const hostCompartment = new Compartment();

/** The href of the link, autolink or bare URL at `pos`, or null. */
export function linkAt(state: EditorState, pos: number): string | null {
  // A `<…>` link destination's URL node keeps the brackets.
  const href = (url: SyntaxNode) => state.sliceDoc(url.from, url.to).replace(/^<(.*)>$/, "$1");
  const node = ancestorAt(state, pos, (n) =>
    n.name === "URL" || n.name === "Link" || n.name === "Autolink",
  );
  const url = node?.name === "URL" ? node : node?.getChild("URL");
  return url ? href(url) : null;
}
