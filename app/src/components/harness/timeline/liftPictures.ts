import type { TextPart } from "@/lib/files/openFile";

/**
 * A question's attached pictures, lifted out to draw above the words, and the
 * prose with the gaps they left closed up. Swept here, not in
 * `splitLibraryPaths`, which the composer shares and must round-trip exactly.
 */
export function liftPictures(parts: TextPart[]): [Extract<TextPart, { kind: "image" }>[], TextPart[]] {
  const pictures = parts.filter((p) => p.kind === "image");
  if (!pictures.length) return [pictures, parts];

  // Prose split only by a picture joins with one space, or none across a break.
  const body: TextPart[] = [];
  for (const p of parts) {
    if (p.kind === "image") continue;
    const last = body[body.length - 1];
    if (p.kind === "text" && last?.kind === "text") {
      const left = last.text.replace(/[ \t]+$/, "");
      const right = p.text.replace(/^[ \t]+/, "");
      const gap = !left || !right || /\n\s*$/.test(left) || /^\s*\n/.test(right) ? "" : " ";
      body[body.length - 1] = { kind: "text", text: left + gap + right };
    } else {
      body.push(p);
    }
  }

  // The composer writes attachment paths on their own trailing line.
  const first = body[0];
  if (first?.kind === "text") body[0] = { kind: "text", text: first.text.replace(/^\s+/, "") };
  const last = body[body.length - 1];
  if (last?.kind === "text")
    body[body.length - 1] = { kind: "text", text: last.text.replace(/\s+$/, "") };

  return [pictures, body.filter((p) => p.kind !== "text" || p.text.length > 0)];
}
