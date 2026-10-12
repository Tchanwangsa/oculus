import { isVideoFile } from "@/lib/files/fileTypes";

/** The tag a card stands for, attributes in the converter's order, empty ones
 *  left out, for the card's `data-md` (`CardShell`). */
export function tagSource(tag: string, attrs: [name: string, value: string | undefined][]): string {
  const escape = (v: string) => v.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
  const written = attrs
    .filter((a): a is [string, string] => !!a[1])
    .map(([name, value]) => ` ${name}="${escape(value)}"`)
    .join("");
  return `<${tag}${written}>\n</${tag}>`;
}

/** A video's name without its video extension; `fallback` when that leaves nothing. */
export function videoDisplayName(name: string | undefined, fallback: string): string {
  const trimmed = name?.trim() ?? "";
  return (isVideoFile(trimmed) ? trimmed.replace(/\.[^.]+$/, "") : trimmed) || fallback;
}
