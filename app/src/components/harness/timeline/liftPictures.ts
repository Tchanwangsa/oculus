import { splitLibraryPaths, type TextPart } from "@/lib/files/libraryPaths";
import { attachmentPath, distinctPaths, splitAttachments } from "@/lib/harness/attachments";

/**
 * A question's pictures, to draw in a row above the words, and its prose. A
 * picture or file the sentence names stays in the prose as a chip, and its
 * picture is in the row too; a trailing line of picture paths (a shelf
 * picture no chip named, and how older messages carried every picture) is
 * lifted out of the prose entirely, its edges closed up as before. Swept
 * here, not in `splitLibraryPaths`, which the composer shares and must
 * round-trip exactly. Pictures are data-directory-relative, in order, once.
 */
export function liftPictures(words: string): { pictures: string[]; prose: TextPart[] } {
  const { text, paths } = splitAttachments(words);
  const parts = splitLibraryPaths(text);
  const inline = parts.flatMap((p) => (p.kind === "image" ? [p.path] : []));
  const trailing = paths.flatMap((p) => attachmentPath(p) ?? []);
  const pictures = distinctPaths([...inline, ...trailing]);
  if (!trailing.length) return { pictures, prose: parts };

  const prose = [...parts];
  const first = prose[0];
  if (first?.kind === "text") prose[0] = { kind: "text", text: first.text.replace(/^\s+/, "") };
  const last = prose[prose.length - 1];
  if (last?.kind === "text") prose[prose.length - 1] = { kind: "text", text: last.text.replace(/\s+$/, "") };
  return { pictures, prose: prose.filter((p) => p.kind !== "text" || p.text.length > 0) };
}
