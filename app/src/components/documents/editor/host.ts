import { Compartment, Facet, type EditorState } from "@codemirror/state";
import { syntaxTree } from "@codemirror/language";
import type { SyntaxNode } from "@lezer/common";

import { isWebUrl, openExternal } from "@/lib/browser";
import type { DbFile } from "@/lib/db";
import { libraryLinkTarget } from "@/lib/libraryLinks";
import { libraryPath, openFileSmart, openLibraryPath } from "@/lib/openFile";

/** What the editor needs from the page around it: picture URLs, where a
 *  link goes, and the note's subject and path for `@` links (`mentions.ts`).
 *  Reconfigured through `hostCompartment` when the data dir or the note's
 *  path changes, which re-resolves every picture. */
export interface NoteHost {
  imageSrc(src: string): string;
  openLink(href: string): void;
  subjectId: number | null;
  /** Library path of the note, `courses/<code>/documents/<title>.md`. */
  notePath: string;
}

export const noteHost = Facet.define<NoteHost, NoteHost>({
  combine: (values) =>
    values[0] ?? { imageSrc: () => "", openLink: () => {}, subjectId: null, notePath: "" },
});

export const hostCompartment = new Compartment();

/** A link in a note: a library file (the same resolution `FileViewer` uses)
 *  opens in the side panel, a web URL in the in-app browser. */
export function openNoteLink(href: string, files: DbFile[]): void {
  const target = libraryLinkTarget(href, files);
  if (target) {
    openFileSmart(target);
    return;
  }
  const lib = libraryPath(href);
  if (lib) {
    openLibraryPath(lib);
    return;
  }
  const url = /^www\./i.test(href) ? `https://${href}` : href;
  if (isWebUrl(url)) void openExternal(url);
}

/** The href of the link, autolink or bare URL at `pos`, or null. */
export function linkAt(state: EditorState, pos: number): string | null {
  // A `<…>` link destination's URL node keeps the brackets.
  const href = (url: SyntaxNode) => state.sliceDoc(url.from, url.to).replace(/^<(.*)>$/, "$1");
  for (const side of [1, -1] as const) {
    let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, side);
    for (; node; node = node.parent) {
      if (node.name === "URL") return href(node);
      if (node.name === "Link" || node.name === "Autolink") {
        const url = node.getChild("URL");
        return url ? href(url) : null;
      }
    }
  }
  return null;
}
